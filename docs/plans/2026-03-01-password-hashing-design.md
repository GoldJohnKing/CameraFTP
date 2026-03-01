# 密码安全存储设计

## 概述

将 FTP 服务器认证密码从明文存储改为 Argon2id 哈希存储，提升安全性。

## 当前问题

- 密码明文存储在 `config.json` 文件中
- 配置文件泄露会直接暴露密码
- 不符合密码存储安全最佳实践

## 设计方案

### 1. 数据结构变更

**Rust 后端 (`src-tauri/src/config.rs`)：**

```rust
pub struct AuthConfig {
    pub anonymous: bool,
    pub username: String,
    pub password_hash: String,  // Argon2id 哈希（Base64 编码）
    pub password_salt: String,  // Salt（Base64 编码）
}
```

**TypeScript 前端：**

```typescript
interface AuthConfig {
  anonymous: boolean;
  username: string;
  passwordHash: string;  // 前端只接收空字符串，不暴露哈希值
  passwordSalt: string;
}
```

### 2. 密码处理流程

**保存密码：**
1. 用户在前端输入密码
2. 通过 Tauri IPC 明文传输到后端
3. 后端生成随机 salt
4. 使用 Argon2id 计算哈希
5. 存储 `password_hash` + `password_salt`
6. 返回成功，不返回任何密码相关信息

**认证流程：**
1. FTP 客户端连接，提供用户名和密码
2. 后端从配置读取 `password_hash` 和 `password_salt`
3. 使用 Argon2id 验证：`verify(input_password, stored_hash, salt)`
4. 验证通过则允许访问

### 3. 前端交互设计

**输入框状态：**

| 场景 | 显示内容 | 预览图标 |
|------|----------|----------|
| 新配置（无已保存密码） | 空白 | ✅ 可预览输入 |
| 编辑已保存配置 | `••••••••` 占位符 | ❌ 隐藏 |
| 用户修改密码框 | 用户输入内容 | ✅ 可预览输入 |

**实现逻辑：**
- `passwordInput` 初始值：新配置时为空，已有配置时为占位符 `••••••••`
- 用户开始输入时，清除占位符，启用预览
- 保存时，只有当输入值 ≠ 占位符时才发送到后端

### 4. 安全参数

| 参数 | 值 | 说明 |
|------|-----|------|
| 算法 | Argon2id | 抗 GPU/ASIC 攻击，内存困难 |
| 内存成本 | 64 MB | 平衡安全与性能 |
| 时间成本 | 3 迭代 | 约 100ms 验证时间 |
| 并行度 | 4 线程 | 利用多核 |
| Salt 长度 | 16 字节 | 防彩虹表 |
| 输出长度 | 32 字节 | 256 位哈希 |

### 5. 组件变更

**后端：**
- `src-tauri/src/config.rs` - 修改 `AuthConfig` 结构
- `src-tauri/src/crypto.rs` - 新增，密码哈希工具模块
- `src-tauri/src/ftp/types.rs` - 修改 `FtpAuthConfig` 处理逻辑
- `src-tauri/src/ftp/server.rs` - 修改认证器使用哈希验证

**前端：**
- `src/components/AdvancedConnectionConfig.tsx` - 修改密码输入逻辑
- `src/types/index.ts` - 更新类型定义

### 6. 依赖

```toml
# Cargo.toml
[dependencies]
argon2 = "0.5"
rand = "0.8"  # 用于生成 salt
```

## 风险评估

| 风险 | 缓解措施 |
|------|----------|
| 用户忘记密码 | 提供「重置密码」功能，清除哈希重新设置 |
| 哈希计算耗时 | Argon2id 参数调优，目标 100ms |
| 配置迁移 | 不需要向后兼容，直接使用新格式 |

## 验收标准

1. ✅ 密码不再以明文存储在配置文件中
2. ✅ FTP 认证功能正常工作
3. ✅ 用户输入时可预览密码
4. ✅ 保存后无法预览已保存密码
5. ✅ 配置文件泄露不会暴露原始密码
