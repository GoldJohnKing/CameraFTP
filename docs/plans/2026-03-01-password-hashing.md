# 密码哈希存储实现计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 将 FTP 认证密码从明文存储改为 Argon2id 哈希存储，同时保留输入时的预览功能。

**Architecture:** 后端使用 Argon2id 算法对密码进行哈希存储，前端在输入时可预览但保存后无法查看原始密码。

**Tech Stack:** Rust (argon2 crate), TypeScript/React, Tauri IPC

---

## Task 1: 添加密码哈希依赖

**Files:**
- Modify: `src-tauri/Cargo.toml`

**Step 1: 添加 argon2 和 rand 依赖**

在 `[dependencies]` 部分添加：

```toml
argon2 = "0.5"
rand = "0.8"
```

**Step 2: 验证依赖可编译**

Run: `cd src-tauri && cargo check`
Expected: 无错误

**Step 3: Commit**

```bash
git add src-tauri/Cargo.toml
git commit -m "chore: add argon2 and rand dependencies for password hashing"
```

---

## Task 2: 创建密码哈希工具模块

**Files:**
- Create: `src-tauri/src/crypto.rs`
- Modify: `src-tauri/src/lib.rs`

**Step 1: 创建 crypto.rs 模块**

```rust
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rand::RngCore;

/// Argon2id 参数配置
const MEMORY_COST: u32 = 65536; // 64 MB
const TIME_COST: u32 = 3;
const PARALLELISM: u32 = 4;
const SALT_LENGTH: usize = 16;
const OUTPUT_LENGTH: usize = 32;

/// 密码哈希结果
#[derive(Debug, Clone)]
pub struct HashedPassword {
    pub hash: String,
    pub salt: String,
}

/// 对密码进行哈希
pub fn hash_password(password: &str) -> HashedPassword {
    // 生成随机 salt
    let mut salt_bytes = [0u8; SALT_LENGTH];
    OsRng.fill_bytes(&mut salt_bytes);
    let salt = BASE64.encode(salt_bytes);

    // 创建 Argon2id 实例
    let argon2 = Argon2::new(
        argon2::Algorithm::Argon2id,
        argon2::Version::V0x13,
        argon2::Params::new(MEMORY_COST, TIME_COST, PARALLELISM, Some(OUTPUT_LENGTH))
            .expect("Invalid Argon2 parameters"),
    );

    // 使用自定义 salt 进行哈希
    let salt_string = SaltString::encode_b64(&salt_bytes).expect("Failed to encode salt");
    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt_string)
        .expect("Failed to hash password");

    HashedPassword {
        hash: password_hash.to_string(),
        salt,
    }
}

/// 验证密码
pub fn verify_password(password: &str, stored_hash: &str) -> bool {
    let parsed_hash = match PasswordHash::new(stored_hash) {
        Ok(h) => h,
        Err(_) => return false,
    };

    let argon2 = Argon2::new(
        argon2::Algorithm::Argon2id,
        argon2::Version::V0x13,
        argon2::Params::new(MEMORY_COST, TIME_COST, PARALLELISM, Some(OUTPUT_LENGTH))
            .expect("Invalid Argon2 parameters"),
    );

    argon2
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_and_verify() {
        let password = "test_password_123";
        let hashed = hash_password(password);

        assert!(!hashed.hash.is_empty());
        assert!(!hashed.salt.is_empty());
        assert!(verify_password(password, &hashed.hash));
        assert!(!verify_password("wrong_password", &hashed.hash));
    }

    #[test]
    fn test_different_salts() {
        let password = "same_password";
        let hash1 = hash_password(password);
        let hash2 = hash_password(password);

        // 相同密码应产生不同的哈希值
        assert_ne!(hash1.hash, hash2.hash);
        assert_ne!(hash1.salt, hash2.salt);
    }
}
```

**Step 2: 在 lib.rs 中注册模块**

在 `src-tauri/src/lib.rs` 顶部添加：

```rust
mod crypto;
```

**Step 3: 运行测试验证**

Run: `cd src-tauri && cargo test crypto::tests --lib`
Expected: 2 tests passed

**Step 4: Commit**

```bash
git add src-tauri/src/crypto.rs src-tauri/src/lib.rs
git commit -m "feat: add password hashing module with Argon2id"
```

---

## Task 3: 修改 AuthConfig 数据结构

**Files:**
- Modify: `src-tauri/src/config.rs`

**Step 1: 修改 AuthConfig 结构体**

将 `src-tauri/src/config.rs` 中的 `AuthConfig` 从：

```rust
pub struct AuthConfig {
    pub anonymous: bool,
    pub username: String,
    pub password: String,
}
```

改为：

```rust
pub struct AuthConfig {
    pub anonymous: bool,
    pub username: String,
    pub password_hash: String,
    pub password_salt: String,
}
```

**Step 2: 修改 Default 实现**

```rust
impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            anonymous: true,
            username: String::new(),
            password_hash: String::new(),
            password_salt: String::new(),
        }
    }
}
```

**Step 3: 添加 ts-rs 导出配置**

确保 `AuthConfig` 有正确的 ts-rs 导出：

```rust
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", default)]
pub struct AuthConfig {
    pub anonymous: bool,
    pub username: String,
    #[ts(type = "string")]
    pub password_hash: String,
    #[ts(type = "string")]
    pub password_salt: String,
}
```

**Step 4: 验证编译**

Run: `./build.sh windows`
Expected: 编译成功（可能有其他文件的错误，后续修复）

**Step 5: Commit**

```bash
git add src-tauri/src/config.rs
git commit -m "refactor: change AuthConfig to use password_hash and password_salt"
```

---

## Task 4: 修改 FtpAuthConfig 和认证逻辑

**Files:**
- Modify: `src-tauri/src/ftp/types.rs`
- Modify: `src-tauri/src/ftp/server.rs`

**Step 1: 修改 FtpAuthConfig 结构**

在 `src-tauri/src/ftp/types.rs` 中，将 `FtpAuthConfig` 改为：

```rust
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FtpAuthConfig {
    pub anonymous: bool,
    pub username: String,
    pub password_hash: String,
}
```

**Step 2: 修改 Default 实现**

```rust
impl Default for FtpAuthConfig {
    fn default() -> Self {
        Self {
            anonymous: true,
            username: String::new(),
            password_hash: String::new(),
        }
    }
}
```

**Step 3: 修改 From<&AuthConfig> 实现**

```rust
impl From<&AuthConfig> for FtpAuthConfig {
    fn from(auth: &AuthConfig) -> Self {
        let should_be_anonymous = auth.anonymous
            || auth.username.trim().is_empty()
            || auth.password_hash.is_empty();

        Self {
            anonymous: should_be_anonymous,
            username: auth.username.clone(),
            password_hash: auth.password_hash.clone(),
        }
    }
}
```

**Step 4: 修改 CustomAuthenticator 认证逻辑**

在 `src-tauri/src/ftp/server.rs` 中，修改 `CustomAuthenticator::authenticate` 方法：

```rust
#[async_trait::async_trait]
impl Authenticator for CustomAuthenticator {
    async fn authenticate(
        &self,
        username: &str,
        creds: &Credentials,
    ) -> Result<Principal, AuthenticationError> {
        if self.auth_config.anonymous {
            return Ok(Principal {
                username: username.to_string(),
            });
        }

        // 验证用户名
        if username != self.auth_config.username {
            return Err(AuthenticationError::BadPassword);
        }

        // 使用 Argon2id 验证密码
        let password = creds.password.as_deref().unwrap_or("");
        
        if crate::crypto::verify_password(password, &self.auth_config.password_hash) {
            Ok(Principal {
                username: username.to_string(),
            })
        } else {
            Err(AuthenticationError::BadPassword)
        }
    }
}
```

**Step 5: 验证编译**

Run: `./build.sh windows`
Expected: 编译成功

**Step 6: Commit**

```bash
git add src-tauri/src/ftp/types.rs src-tauri/src/ftp/server.rs
git commit -m "refactor: update FTP auth to use password hash verification"
```

---

## Task 5: 添加密码哈希保存命令

**Files:**
- Modify: `src-tauri/src/commands.rs`

**Step 1: 添加保存密码的命令**

在 `src-tauri/src/commands.rs` 中添加新命令：

```rust
use crate::crypto;

/// 保存认证配置（对密码进行哈希）
#[tauri::command]
pub async fn save_auth_config(
    state: tauri::State<'_, std::sync::Arc<std::sync::Mutex<AppConfig>>>,
    anonymous: bool,
    username: String,
    password: String,
) -> Result<(), String> {
    let mut config = state.lock().map_err(|e| e.to_string())?;
    
    let (password_hash, password_salt) = if anonymous || password.is_empty() {
        (String::new(), String::new())
    } else {
        let hashed = crypto::hash_password(&password);
        (hashed.hash, hashed.salt)
    };
    
    config.advanced_connection.auth = AuthConfig {
        anonymous,
        username,
        password_hash,
        password_salt,
    };
    
    config.save().map_err(|e| e.to_string())?;
    
    tracing::info!("Auth config saved with hashed password");
    Ok(())
}
```

**Step 2: 注册命令**

在 `src-tauri/src/lib.rs` 的 `invoke_handler` 中添加：

```rust
.invoke_handler(tauri::generate_handler![
    // ... 其他命令
    save_auth_config,
])
```

**Step 3: 添加必要的 use 语句**

确保 `commands.rs` 顶部有：

```rust
use crate::config::{AppConfig, AuthConfig};
use crate::crypto;
```

**Step 4: 验证编译**

Run: `./build.sh windows`
Expected: 编译成功

**Step 5: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat: add save_auth_config command with password hashing"
```

---

## Task 6: 更新前端类型定义

**Files:**
- Modify: `src/types/index.ts`

**Step 1: 更新 AuthConfig 接口**

```typescript
export interface AuthConfig {
  anonymous: boolean;
  username: string;
  passwordHash: string;
  passwordSalt: string;
}
```

**Step 2: Commit**

```bash
git add src/types/index.ts
git commit -m "refactor: update AuthConfig type for password hashing"
```

---

## Task 7: 修改前端密码输入组件

**Files:**
- Modify: `src/components/AdvancedConnectionConfig.tsx`

**Step 1: 添加密码占位符常量**

```typescript
const PASSWORD_PLACEHOLDER = '••••••••';
```

**Step 2: 修改密码输入状态初始化**

```typescript
const [passwordInput, setPasswordInput] = useState(() => {
  // 如果已有哈希密码，显示占位符
  if (config.auth.passwordHash && !config.auth.anonymous) {
    return PASSWORD_PLACEHOLDER;
  }
  return '';
});
const [hasExistingPassword, setHasExistingPassword] = useState(
  !!config.auth.passwordHash && !config.auth.anonymous
);
```

**Step 3: 修改密码变更处理**

```typescript
const handlePasswordChange = (e: React.ChangeEvent<HTMLInputElement>) => {
  const value = e.target.value;
  // 用户开始输入，清除占位符状态
  if (hasExistingPassword && value !== PASSWORD_PLACEHOLDER) {
    setHasExistingPassword(false);
  }
  setPasswordInput(value);
};
```

**Step 4: 修改密码失焦保存逻辑**

```typescript
const handlePasswordBlur = async () => {
  // 如果是占位符或未修改，不保存
  if (passwordInput === PASSWORD_PLACEHOLDER || passwordInput === '') {
    return;
  }
  
  // 调用新的保存命令
  try {
    await invoke('save_auth_config', {
      anonymous: config.auth.anonymous,
      username: usernameInput,
      password: passwordInput,
    });
    // 保存成功后，切换到占位符显示
    setPasswordInput(PASSWORD_PLACEHOLDER);
    setHasExistingPassword(true);
    setShowPassword(false);
  } catch (error) {
    console.error('Failed to save auth config:', error);
  }
};
```

**Step 5: 修改密码预览按钮显示逻辑**

```tsx
{/* 只有在输入新密码时才显示预览按钮 */}
{(!hasExistingPassword || passwordInput !== PASSWORD_PLACEHOLDER) && (
  <button
    type="button"
    className="..."
    onClick={() => setShowPassword(!showPassword)}
  >
    {showPassword ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
  </button>
)}
```

**Step 6: 验证前端编译**

Run: `./build.sh frontend`
Expected: 编译成功

**Step 7: Commit**

```bash
git add src/components/AdvancedConnectionConfig.tsx
git commit -m "feat: update password input to support hash storage"
```

---

## Task 8: 更新 InfoCard 组件

**Files:**
- Modify: `src/components/InfoCard.tsx`

**Step 1: 修改密码检测逻辑**

将：

```typescript
!advanced.auth?.password
```

改为：

```typescript
!advanced.auth?.passwordHash
```

**Step 2: Commit**

```bash
git add src/components/InfoCard.tsx
git commit -m "fix: update InfoCard to check passwordHash instead of password"
```

---

## Task 9: 更新 serverStore

**Files:**
- Modify: `src/stores/serverStore.ts`

**Step 1: 检查并更新密码相关引用**

确保任何引用 `password` 字段的地方都更新为 `passwordHash`。

**Step 2: Commit**

```bash
git add src/stores/serverStore.ts
git commit -m "fix: update serverStore for password hash changes"
```

---

## Task 10: 完整验证

**Step 1: 构建所有平台**

Run: `./build.sh windows && ./build.sh android`
Expected: 全部编译成功

**Step 2: 手动测试清单**

- [ ] 新安装：配置匿名模式 → 保存 → 配置文件无密码字段
- [ ] 新安装：配置用户名密码 → 保存 → 配置文件有 passwordHash
- [ ] 输入密码时可点击眼睛图标预览
- [ ] 保存后密码显示为占位符，无预览按钮
- [ ] 重启应用后密码仍为占位符
- [ ] FTP 连接使用正确密码可登录
- [ ] FTP 连接使用错误密码被拒绝

**Step 3: 最终 Commit**

```bash
git add -A
git commit -m "feat: complete password hashing implementation"
```

---

## 注意事项

1. **不要使用 `lsp_diagnostics`** - 始终通过 `./build.sh` 编译验证
2. **TypeScript 类型同步** - Rust 的 `password_hash` → TypeScript 的 `passwordHash`
3. **向后兼容** - 本实现不处理旧版明文密码配置
