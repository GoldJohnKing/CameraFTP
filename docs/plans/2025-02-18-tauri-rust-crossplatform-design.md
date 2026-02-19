# Camera FTP Companion - 跨平台架构设计文档

**日期**: 2025-02-18  
**架构**: Tauri + Rust  
**目标平台**: Windows, Android

---

## 1. 项目概述

### 1.1 功能目标
实现一款傻瓜式相机FTP伴侣应用，支持Windows和Android双平台：
- 内置FTP服务器，接收相机上传的照片
- 实时显示连接状态和传输统计
- 极简配置，一键开关
- 支持后台运行

### 1.2 设计理念
**"开箱即用"原则**:
- 无需手动配置IP、端口
- 自动检测可用网络接口
- 智能推荐端口（默认21，被占用则自动选择）
- 零学习成本

---

## 2. 系统架构

### 2.1 整体架构图

```
┌─────────────────────────────────────────────────────────────┐
│                      UI Layer (Web)                          │
│              React + TypeScript + TailwindCSS               │
│                                                              │
│   ┌──────────────┐  ┌──────────────┐  ┌──────────────┐     │
│   │ ServerCard   │  │  StatsCard   │  │  InfoCard    │     │
│   │   (开关)      │  │ (传输统计)    │  │ (连接信息)    │     │
│   └──────────────┘  └──────────────┘  └──────────────┘     │
└────────────────────────────┬────────────────────────────────┘
                             │ Tauri IPC (Command / Event)
┌────────────────────────────┴────────────────────────────────┐
│                   Tauri Runtime                              │
│                                                              │
│  ┌─────────────────┐          ┌──────────────────────────┐  │
│  │   Windows       │          │       Android            │  │
│  │  (WebView2)     │          │   (WKWebView)            │  │
│  │                 │          │                          │  │
│  │  ┌───────────┐  │          │  ┌──────────────────┐    │  │
│  │  │ System    │  │          │  │  Foreground      │    │  │
│  │  │ Tray      │  │          │  │  Service         │    │  │
│  │  └───────────┘  │          │  │  (保活)          │    │  │
│  └─────────────────┘          │  └──────────────────┘    │  │
│                               └──────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                             │
┌────────────────────────────┴────────────────────────────────┐
│              Rust Core Library (src-tauri/src)               │
│                                                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐   │
│  │ FtpServer    │  │   Stats      │  │    Network       │   │
│  │   Module     │  │  Collector   │  │   Manager        │   │
│  │              │  │              │  │                  │   │
│  │ - Passive    │  │ - Conn count │  │ - IP detection   │   │
│  │   mode only  │  │ - File count │  │ - Port check     │   │
│  │ - Single     │  │ - Byte count │  │ - Interface      │   │
│  │   session    │  │ - Last file  │  │   listing        │   │
│  └──────────────┘  └──────────────┘  └──────────────────┘   │
│                                                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐   │
│  │   Storage    │  │   Config     │  │   File Handler   │   │
│  │  (保存路径)   │  │   Manager    │  │   (自动打开)     │   │
│  └──────────────┘  └──────────────┘  └──────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 技术栈选型

| 层级 | 技术 | 版本 | 说明 |
|------|------|------|------|
| **构建工具** | Tauri CLI | v2.x | 跨平台构建 |
| **前端框架** | React | v18 | UI组件 |
| **前端语言** | TypeScript | v5 | 类型安全 |
| **样式** | TailwindCSS | v3 | 原子化CSS |
| **后端语言** | Rust | v1.75+ | 核心实现 |
| **异步运行时** | Tokio | v1.x | 异步IO |
| **FTP协议** | 自实现 | - | 精简版，仅上传 |
| **序列化** | Serde | v1.x | 配置持久化 |
| **移动端** | Tauri Mobile | v2.x | Android支持 |

---

## 3. 模块详细设计

### 3.1 FTP服务器模块 (ftp_server.rs)

**设计原则**: 极简实现，仅支持相机上传场景

```rust
pub struct FtpServer {
    listener: TcpListener,
    config: ServerConfig,
    state: Arc<ServerState>,
}

pub struct ServerConfig {
    pub port: u16,
    pub root_path: PathBuf,
    pub allow_anonymous: bool,  // 固定为 true
}

pub struct ServerState {
    pub is_running: AtomicBool,
    pub connected_clients: AtomicUsize,
    pub files_received: AtomicU64,
    pub bytes_received: AtomicU64,
    pub last_file: Mutex<Option<String>>,
}
```

**支持的FTP命令** (最小集):
- `USER` / `PASS` - 匿名登录
- `PWD` / `CWD` - 目录切换
- `TYPE` / `MODE` / `STRU` - 传输模式
- `PASV` - 被动模式（主要使用）
- `STOR` - 上传文件（核心功能）
- `QUIT` - 断开连接

**不支持的功能**:
- PORT模式（主动模式）
- 下载（RETR）
- 删除（DELE）
- 多用户认证
- TLS/SSL（FTP over TLS）

### 3.2 统计收集器 (stats.rs)

```rust
pub struct StatsCollector {
    inner: Arc<StatsInner>,
}

pub struct StatsSnapshot {
    pub is_running: bool,
    pub port: u16,
    pub server_ip: String,
    pub connected_clients: usize,
    pub files_received: u64,
    pub bytes_received: u64,
    pub last_file: Option<String>,
    pub last_file_preview: Option<String>, // Base64缩略图
}

// 实时推送到前端
pub enum StatsEvent {
    ClientConnected { ip: String },
    ClientDisconnected { ip: String },
    FileReceived { filename: String, size: u64 },
    ServerStarted { ip: String, port: u16 },
    ServerStopped,
}
```

### 3.3 网络管理器 (network.rs)

**自动检测可用IP**:
```rust
pub struct NetworkManager;

impl NetworkManager {
    /// 获取本机所有可用IP地址
    pub fn list_interfaces() -> Vec<InterfaceInfo>;
    
    /// 推荐最佳IP（优先WiFi/以太网，过滤虚拟网卡）
    pub fn recommended_ip() -> Option<String>;
    
    /// 检查端口是否可用
    pub fn is_port_available(port: u16) -> bool;
    
    /// 自动选择可用端口（从默认开始递增）
    pub fn find_available_port(start: u16) -> Option<u16>;
}

pub struct InterfaceInfo {
    pub name: String,
    pub ip: String,
    pub is_wifi: bool,
    pub is_ethernet: bool,
    pub is_up: bool,
}
```

### 3.4 配置管理 (config.rs)

```rust
#[derive(Serialize, Deserialize)]
pub struct AppConfig {
    pub save_path: PathBuf,
    pub auto_open: bool,
    pub auto_open_program: Option<String>,
    pub port: u16,  // 默认21，0表示自动
    pub file_extensions: Vec<String>, // 默认 ["jpg", "jpeg", "raw", "png"]
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            save_path: default_pictures_dir(),
            auto_open: true,
            auto_open_program: None,
            port: 21,
            file_extensions: vec![
                "jpg".to_string(),
                "jpeg".to_string(),
                "raw".to_string(),
                "png".to_string(),
            ],
        }
    }
}
```

### 3.5 平台适配层

#### Windows 特有功能
```rust
// tray.rs
pub fn setup_tray(app: &mut App) -> Result<()>;
pub fn show_notification(title: &str, body: &str);
pub fn open_file_explorer(path: &Path);
pub fn open_with_default_program(path: &Path);
```

#### Android 特有功能
```rust
// foreground_service.rs
pub fn start_foreground_service(app: &AppHandle);
pub fn stop_foreground_service(app: &AppHandle);
pub fn update_notification(stats: &StatsSnapshot);
```

---

## 4. UI设计规范

### 4.1 布局结构

```
┌─────────────────────────────────────┐
│          Camera FTP Companion       │
│              图传伴侣               │
├─────────────────────────────────────┤
│  ┌───────────────────────────────┐  │
│  │  🔴 服务器已停止              │  │  ← ServerCard
│  │     点击启动接收照片          │  │
│  │                               │  │
│  │     [   启  动   ]            │  │
│  └───────────────────────────────┘  │
├─────────────────────────────────────┤
│  ┌───────────────────────────────┐  │
│  │  📊 传输统计                  │  │  ← StatsCard
│  │                               │  │
│  │  已连接相机: 0               │  │
│  │  已接收照片: 0 张            │  │
│  │  总数据量: 0 MB              │  │
│  │                               │  │
│  │  最新照片: --                │  │
│  │  [缩略图预览区]              │  │
│  └───────────────────────────────┘  │
├─────────────────────────────────────┤
│  ┌───────────────────────────────┐  │
│  │  📡 连接信息                  │  │  ← InfoCard
│  │                               │  │
│  │  协议: FTP (被动模式)        │  │
│  │  地址: 192.168.1.100:21      │  │
│  │  用户名: anonymous           │  │
│  │  密码: (任意)                │  │
│  │                               │  │
│  │  [复制连接信息]              │  │
│  └───────────────────────────────┘  │
├─────────────────────────────────────┤
│  ⚙️ [设置]  ← 折叠面板              │
│     - 保存路径                      │
│     - 自动打开照片                  │
│     - 文件类型过滤                  │
│     - 端口号                        │
└─────────────────────────────────────┘
```

### 4.2 交互规范

| 状态 | 视觉表现 | 说明 |
|------|----------|------|
| 停止 | 红色圆点 + "服务器已停止" | 等待启动 |
| 启动中 | 黄色圆点 + 转圈动画 | 端口绑定中 |
| 运行中 | 绿色圆点 + "运行中" + IP:端口 | 可接收连接 |
| 传输中 | 绿色圆点 + 脉冲动画 + 文件名 | 正在接收文件 |

### 4.3 响应式适配

**桌面端 (Windows)**:
- 窗口大小: 400x600 (可调整)
- 卡片水平居中，最大宽度 400px
- 支持最小化到系统托盘

**移动端 (Android)**:
- 全屏显示
- 卡片占满宽度，边距 16px
- 底部导航栏避让

---

## 5. 数据流设计

### 5.1 启动流程

```
用户点击[启动]
    │
    ▼
┌─────────────────┐
│ 检查保存路径存在 │ ← 不存在则创建
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ 检测网络接口     │ ← 获取可用IP
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ 检查/分配端口   │ ← 被占用则自动+1
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ 启动FTP服务器   │ ← 绑定端口，监听
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│ 更新UI状态      │ ← 显示IP:端口
│ 显示连接信息     │
└─────────────────┘
         │
    ┌────┴────┐
    ▼         ▼
┌───────┐ ┌───────┐
│Windows│ │Android│
│系统托盘│ │前台服务│
└───────┘ └───────┘
```

### 5.2 文件接收流程

```
相机连接
    │
    ▼
FTP STOR 命令
    │
    ▼
创建文件写入流
    │
    ▼
接收数据块 ──→ 写入磁盘
    │
    ▼
文件接收完成
    │
    ├──→ 更新统计 (files_received++, bytes_received += size)
    ├──→ 更新最新文件显示
    ├──→ 生成缩略图预览 (可选)
    ├──→ 触发前端更新 (Tauri Event)
    └──→ 自动打开文件 (如果开启)
```

---

## 6. 错误处理

### 6.1 用户可见错误

| 错误场景 | 处理方式 | 用户提示 |
|----------|----------|----------|
| 端口被占用 | 自动尝试下一个端口 | "端口21被占用，已自动切换至22" |
| 无网络连接 | 禁止启动 | "请连接WiFi或开启热点" |
| 保存路径无权限 | 尝试创建失败 | "无法访问保存路径，请检查权限" |
| 存储空间不足 | 接收前检查 | "存储空间不足，请清理后再试" |
| 相机连接超时 | 断开连接 | "相机连接超时，请重试" |

### 6.2 日志记录

```rust
// 使用 tracing crate
use tracing::{info, warn, error, debug};

// 日志级别
// ERROR: 需要用户干预的错误
// WARN:  需要注意但可恢复的问题
// INFO:  关键操作（启动/停止/文件接收）
// DEBUG: 详细调试信息（开发模式开启）
```

---

## 7. 安全考虑

### 7.1 匿名访问风险
**决策**: 保持匿名访问（相机兼容性）

**缓解措施**:
1. 默认只绑定局域网IP（不绑定0.0.0.0）
2. 仅支持上传，不支持下载/删除
3. 单连接限制（同一时间只允许一台相机）
4. 可选：连接IP白名单（高级设置）

### 7.2 文件系统安全
- 严格限制写入路径（配置目录下）
- 文件名规范化（防止目录遍历攻击）
- 覆盖确认（同名文件提示）

---

## 8. 扩展性考虑

### 8.1 未来可能的功能
- [ ] 多相机连接支持
- [ ] 实时预览（WebRTC流传输）
- [ ] 云存储同步
- [ ] 照片自动分类
- [ ] 元数据提取（EXIF）

### 8.2 架构预留
- FTP模块设计为可替换（trait抽象）
- 统计系统支持插件式扩展
- 配置系统支持版本迁移

---

## 9. 开发规范

### 9.1 代码组织

```
camera-ftp-companion/
├── src/                      # React前端
│   ├── components/           # UI组件
│   │   ├── ServerCard.tsx
│   │   ├── StatsCard.tsx
│   │   ├── InfoCard.tsx
│   │   └── Settings.tsx
│   ├── hooks/                # 自定义Hooks
│   │   └── useFtpServer.ts
│   ├── types/                # TypeScript类型
│   │   └── index.ts
│   ├── utils/                # 工具函数
│   ├── App.tsx
│   └── main.tsx
├── src-tauri/                # Rust后端
│   ├── src/
│   │   ├── main.rs           # 入口
│   │   ├── lib.rs            # 库导出
│   │   ├── commands.rs       # Tauri命令
│   │   ├── ftp/              # FTP模块
│   │   │   ├── mod.rs
│   │   │   ├── server.rs
│   │   │   ├── session.rs
│   │   │   └── protocol.rs
│   │   ├── stats.rs          # 统计
│   │   ├── network.rs        # 网络
│   │   ├── config.rs         # 配置
│   │   └── platform/         # 平台特有
│   │       ├── windows.rs
│   │       └── android.rs
│   ├── Cargo.toml
│   └── tauri.conf.json
├── public/                   # 静态资源
└── package.json
```

### 9.2 命名规范

- **Rust**: `snake_case` (函数/变量), `PascalCase` (类型), `SCREAMING_SNAKE_CASE` (常量)
- **TypeScript**: `camelCase` (函数/变量), `PascalCase` (组件/类型)
- **文件**: `kebab-case.ts` 或 `snake_case.rs`

---

## 10. 待决策事项

1. **缩略图生成**: 是否需要在Rust端生成Base64缩略图？还是前端懒加载？
2. **自动打开**: Windows端如何优雅地打开图片（系统默认程序）？
3. **深色模式**: 是否支持？如何实现（Tailwind dark mode）？
4. **多语言**: 仅中文还是支持英文切换？

---

**文档版本**: 1.0  
**作者**: AI Assistant  
**状态**: 待评审
