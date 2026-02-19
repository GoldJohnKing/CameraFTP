# FTP 服务器改造方案

## 1. 现状分析

### 1.1 当前实现
- **类型**: 自实现 FTP 服务器
- **代码位置**: `src-tauri/src/ftp/` 目录
- **文件**: 
  - `mod.rs` - 模块导出
  - `server.rs` - TCP 服务器和连接管理
  - `session.rs` - 单会话处理（命令解析、数据连接）
  - `protocol.rs` - FTP 命令/响应枚举

### 1.2 已实现的 FTP 命令
```
USER, PASS - 身份验证（任意凭据）
PWD, CWD   - 目录操作
TYPE, MODE, STRU - 传输设置
PASV       - 被动模式
LIST       - 目录列表（刚添加）
STOR       - 文件上传
SYST       - 系统类型
QUIT       - 断开连接
```

### 1.3 当前问题
1. **协议兼容性** - 可能不完全兼容某些 FTP 客户端
2. **功能不完整** - 缺少 RETR（下载）、DELE（删除）等常用命令
3. **维护成本高** - 需要自行实现所有 RFC 959 规范细节
4. **安全性** - 基础实现，缺乏完善的错误处理

---

## 2. 目标架构

### 2.1 选型：libunftp
- **版本**: 0.23.0
- **优势**: 生产就绪、活跃维护、功能完整
- **作者**: bol.com techlab（荷兰最大电商平台）

### 2.2 架构对比

| 维度 | 当前实现 | libunftp |
|------|----------|----------|
| 代码量 | ~400 行自实现 | 使用成熟库 |
| FTP 命令 | 11 个 | 42+ 个 |
| 被动模式 | ✅ | ✅ |
| 主动模式 | ❌ | ✅ |
| FTPS/TLS | ❌ | ✅ |
| 错误处理 | 基础 | RFC 兼容 |
| 维护成本 | 高 | 低 |

---

## 3. 改造步骤

### 步骤 1: 更新依赖
**文件**: `src-tauri/Cargo.toml`

```toml
[dependencies]
# 新增
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
libunftp = { version = "0.23.0", default-features = false }
unftp-sbe-fs = "0.4.0"

# 可选：如果需要 FTPS 支持
# libunftp = { version = "0.23.0", default-features = false, features = ["ring"] }

# 删除（如果不再使用）
# 保持现有: tauri, serde, tracing 等
```

### 步骤 2: 删除自实现 FTP 模块
```bash
rm -rf src-tauri/src/ftp/
```

### 步骤 3: 新建 FTP 包装模块
**文件**: `src-tauri/src/ftp/mod.rs`（新建）

```rust
use libunftp::ServerBuilder;
use unftp_sbe_fs::Filesystem;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex as AsyncMutex;

pub struct FtpServer {
    inner: Option<Arc<libunftp::Server<Filesystem>>>,
    addr: Option<SocketAddr>,
    stats: Arc<ServerStats>,
}

#[derive(Default)]
pub struct ServerStats {
    pub files_received: std::sync::atomic::AtomicU64,
    pub bytes_received: std::sync::atomic::AtomicU64,
    pub last_file: AsyncMutex<Option<String>>,
}

#[derive(Clone, Debug)]
pub struct ServerConfig {
    pub port: u16,
    pub root_path: PathBuf,
    pub allow_anonymous: bool,
}

impl FtpServer {
    pub fn new(config: ServerConfig) -> Self {
        Self {
            inner: None,
            addr: None,
            stats: Arc::new(ServerStats::default()),
        }
    }

    pub async fn start(&mut self, config: ServerConfig) -> Result<SocketAddr, Box<dyn std::error::Error>> {
        let root = config.root_path.clone();
        
        let server = ServerBuilder::new(Box::new(move || {
            Filesystem::new(root.clone()).unwrap()
        }))
        .greeting("Camera FTP Companion")
        .passive_ports(50000..=50100)
        .build()?;

        let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
        
        // 在后台运行服务器
        tokio::spawn(async move {
            server.listen(addr).await;
        });

        self.addr = Some(addr);
        Ok(addr)
    }

    pub fn stop(&self) {
        // libunftp 目前没有优雅的停止方法
        // 需要研究如何实现，或保持当前 socket drop 方式
    }

    pub fn stats(&self) -> Arc<ServerStats> {
        self.stats.clone()
    }
}
```

### 步骤 4: 更新命令模块
**文件**: `src-tauri/src/commands.rs`

**变更点**:
1. 更新 `FtpServerState` 以适配新的 FtpServer
2. 修改 `start_server` 处理 libunftp 的异步启动
3. 修改 `stop_server`（注意：libunftp 的停止机制需要研究）
4. 更新 `get_server_status` 返回数据结构

**关键代码片段**:
```rust
// 修改 start_server
let server = FtpServer::new(server_config);
match server.start().await {
    Ok(addr) => {
        // 存储服务器实例
        // 返回 ServerInfo
    }
}

// 修改 stop_server  
pub async fn stop_server(...) -> Result<(), String> {
    // 需要实现 libunftp 的停止逻辑
    // 可能使用 AbortHandle 或类似的机制
}
```

### 步骤 5: 更新 lib.rs
**文件**: `src-tauri/src/lib.rs`

```rust
pub mod commands;
pub mod config;
pub mod ftp;        // 保持，但内部使用 libunftp
pub mod network;
pub mod platform;

// 保持其他代码不变
```

### 步骤 6: 前端适配检查
- 检查 `ServerInfo` 和 `StatsUpdate` 结构是否变化
- 确认前端接口调用是否需要调整

---

## 4. API 兼容性

### 4.1 保持不变（向后兼容）
```rust
// commands.rs 中的 Tauri 命令
start_server()   // ✅ 保持
stop_server()    // ✅ 保持  
get_server_status() // ⚠️ 可能调整
get_network_info() // ✅ 保持
load_config()    // ✅ 保持
save_config()    // ✅ 保持
check_port_available() // ✅ 保持
```

### 4.2 数据结构变化

**ServerInfo** - 保持不变
```rust
pub struct ServerInfo {
    pub is_running: bool,
    pub ip: String,
    pub port: u16,
    pub url: String,
    pub username: String,
    pub password_info: String,
}
```

**StatsUpdate** - 可能变化
```rust
// 当前
pub struct StatsUpdate {
    pub connected_clients: usize,  // ⚠️ libunftp 可能没有直接提供
    pub files_received: u64,
    pub bytes_received: u64,
    pub last_file: Option<String>,
}
```

---

## 5. 风险与缓解

### 风险 1: libunftp 的停止机制
**问题**: libunftp 的 `Server::listen()` 是阻塞的，没有明显的停止方法。

**缓解方案**:
- 方案 A: 使用 `tokio::select!` + 取消 token
- 方案 B: 使用 `abort_handle` 强制终止任务
- 方案 C: 保持当前设计（drop socket 会终止监听）

### 风险 2: 统计数据获取
**问题**: libunftp 可能不直接暴露 `connected_clients` 等统计信息。

**缓解方案**:
- 实现自定义存储后端（StorageBackend），包装 Filesystem 并添加统计
- 或使用 libunftp 的回调/事件机制（如果有的话）

### 风险 3: 被动端口配置
**问题**: 相机可能使用特定的被动端口范围。

**缓解方案**:
- libunftp 支持 `.passive_ports(50000..=50100)`，可直接配置

### 风险 4: 匿名认证
**问题**: 相机通常使用匿名 FTP。

**缓解方案**:
- libunftp 默认允许匿名访问
- 可通过 `.authenticator()` 自定义认证

---

## 6. 回滚方案

### 6.1 代码回滚
- 使用 Git 分支：`feature/libunftp-migration`
- 保留原 FTP 模块在 `ftp_legacy/` 目录作为备份

### 6.2 快速回滚
```bash
# 如果需要快速回滚
git checkout main -- src-tauri/src/ftp/
# 恢复 Cargo.toml 的依赖
# 重新构建
```

---

## 7. 实施计划

### 阶段 1: 准备（0.5 天）
- [ ] 创建功能分支 `feature/libunftp-migration`
- [ ] 备份当前 FTP 模块到 `ftp_legacy/`
- [ ] 更新 Cargo.toml 依赖

### 阶段 2: 核心改造（1 天）
- [ ] 实现新的 `ftp/mod.rs` 包装器
- [ ] 更新 `commands.rs` 适配新接口
- [ ] 处理服务器停止机制

### 阶段 3: 功能完善（0.5 天）
- [ ] 实现统计信息收集
- [ ] 测试被动模式端口配置
- [ ] 验证匿名认证

### 阶段 4: 测试（1 天）
- [ ] 单元测试：各命令功能
- [ ] 集成测试：真实相机连接
- [ ] 跨平台测试：Windows/Linux

### 阶段 5: 合并（0.5 天）
- [ ] 代码审查
- [ ] 合并到主分支
- [ ] 删除备份代码

**总工期**: 约 3.5 天

---

## 8. 需要进一步研究的问题

1. **libunftp 如何优雅停止？** - 需要查阅文档或源码
2. **如何获取活跃连接数？** - 检查是否有 session 回调
3. **如何在上传完成后获取文件名？** - 可能需要自定义 StorageBackend
4. **编译体积影响？** - 需要测试 release 构建大小

---

## 9. 决策点

### 是否继续改造？

**推荐改造的情况**:
- ✅ 需要更好的协议兼容性
- ✅ 计划添加 FTPS/TLS 支持
- ✅ 希望减少维护成本
- ✅ 需要更多 FTP 功能（下载、删除等）

**暂缓改造的情况**:
- ⏸️ 当前实现已满足所有需求
- ⏸️ 项目时间紧迫（改造需 3-4 天）
- ⏸️ 担心引入新依赖的风险

---

**方案起草日期**: 2026-02-19  
**建议决策人**: 项目负责人评估后决定
