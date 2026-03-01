# 密码安全存储设计

## 概述

采用后端 Argon2id 哈希方案保护 FTP 服务器认证密码。

## 当前问题

- 密码明文存储在 `config.json` 文件中
- 配置文件泄露会直接暴露密码
- 不符合密码存储安全最佳实践

## 设计方案

### 1. 架构

```
用户输入密码
     ↓
IPC 传输（明文，本地通信）
     ↓
┌─────────────────────────────────────────────────────────────┐
│  [后端]                                                      │
│  ├─ 生成随机 Salt (16 字节)                                  │
│  ├─ Argon2id(password, salt, 64MB, 3 迭代) ~100ms           │
│  └─ 存储 Argon2id 哈希 + salt                               │
└─────────────────────────────────────────────────────────────┘
```

### 2. 威胁模型

本方案针对**普通用户威胁模型**：

| 威胁             | 可能性 | 防护措施            |
| ---------------- | ------ | ------------------- |
| 配置文件泄露     | 中     | ✅ Argon2id 哈希    |
| 彩虹表攻击       | 中     | ✅ 随机 Salt        |
| GPU 暴力破解     | 中     | ✅ Argon2id 64MB    |
| IPC 截获         | 低     | ⚠️ 需 root 权限     |
| 设备被 root 攻击 | 极低   | ❌ 超出防护范围     |

### 3. 数据结构

**Rust 后端 (`src-tauri/src/config.rs`)：**

```rust
pub struct AuthConfig {
    pub anonymous: bool,
    pub username: String,
    pub password_hash: String,  // Argon2id 哈希（PHC 格式）
    pub password_salt: String,  // Salt（Base64 编码）
}
```

**TypeScript 前端：**

```typescript
interface AuthConfig {
  anonymous: boolean;
  username: string;
  passwordHash: string;  // 前端只接收空字符串或标记
  passwordSalt: string;
}
```

### 4. 密码处理流程

**保存密码：**
1. 用户在前端输入密码
2. 通过 Tauri IPC 传输明文密码（本地通信）
3. 后端生成随机 salt
4. 使用 Argon2id 哈希密码
5. 存储 Argon2id 哈希 + salt
6. 返回成功（不返回哈希值）

**认证流程：**
1. FTP 客户端连接，提供用户名和密码
2. 后端读取存储的 Argon2id 哈希
3. 使用 Argon2id 验证密码
4. 验证通过则允许访问

### 5. 前端交互设计

**输入框状态：**

| 场景                 | 显示内容        | 预览图标   |
| -------------------- | --------------- | ---------- |
| 新配置（无已保存密码）| 空白            | ✅ 可预览  |
| 编辑已保存配置       | `••••••••` 占位符 | ❌ 隐藏    |
| 用户修改密码框       | 用户输入内容    | ✅ 可预览  |

**保存逻辑：**
1. 用户输入密码
2. 调用后端 `save_auth_config` 命令，传入明文密码
3. 后端 Argon2id 哈希后存储

### 6. 安全参数

**后端 Argon2id：**

| 参数     | 值      | 说明               |
| -------- | ------- | ------------------ |
| 算法     | Argon2id| 抗 GPU/ASIC 攻击   |
| 内存成本 | 64 MB   | 平衡安全与性能     |
| 时间成本 | 3 迭代  | ~100ms 验证时间    |
| 并行度   | 4 线程  | 利用多核           |
| Salt     | 16 字节 | 随机生成，防彩虹表 |
| 输出长度 | 32 字节 | 256 位哈希         |

### 7. 性能

| 操作         | 耗时    |
| ------------ | ------- |
| 保存密码     | ~110ms  |
| FTP 登录认证 | ~100ms  |

### 8. 安全属性

| 攻击向量      | 防护状态      | 说明                   |
| ------------- | ------------- | ---------------------- |
| 配置文件泄露  | ✅ 防护       | Argon2id 64MB 抗 GPU   |
| 彩虹表攻击    | ✅ 防护       | 随机 Salt              |
| 批量攻击      | ✅ 防护       | 每用户唯一 Salt        |
| GPU 暴力破解  | ✅ 防护       | 64MB 内存限制          |
| IPC 截获      | ⚠️ 低风险     | 需要 root/管理员权限   |

### 9. 组件变更

**后端：**
- `src-tauri/Cargo.toml` - 添加 argon2, rand 依赖
- `src-tauri/src/crypto.rs` - 新增，Argon2id 哈希工具
- `src-tauri/src/config.rs` - 修改 `AuthConfig` 结构
- `src-tauri/src/ftp/types.rs` - 修改 `FtpAuthConfig`
- `src-tauri/src/ftp/server.rs` - 修改认证器使用 Argon2id
- `src-tauri/src/commands.rs` - 新增 `save_auth_config` 命令
- `src-tauri/src/lib.rs` - 注册新命令

**前端：**
- `src/components/AdvancedConnectionConfig.tsx` - 修改密码输入逻辑
- `src/components/InfoCard.tsx` - 修改密码检测逻辑
- `src/types/index.ts` - 更新类型定义
- `src/stores/serverStore.ts` - 更新密码字段引用

### 10. 依赖

**Rust (Cargo.toml)：**
```toml
[dependencies]
argon2 = "0.5"
rand = "0.8"
```

**TypeScript：**
- 无需额外依赖

## 风险评估

| 风险           | 缓解措施                             |
| -------------- | ------------------------------------ |
| 用户忘记密码   | 提供「重置密码」功能                 |
| 哈希计算耗时   | ~100ms，用户可接受                   |
| 配置迁移       | 不需要向后兼容，直接使用新格式       |
| IPC 截获       | 需要 root 权限，普通用户风险极低     |

## 验收标准

1. ✅ 密码不再以明文存储在配置文件中
2. ✅ FTP 认证功能正常工作
3. ✅ 用户输入时可预览密码
4. ✅ 保存后无法预览已保存密码
5. ✅ 配置文件泄露不会暴露原始密码
6. ✅ 性能在 ~110ms 以内
