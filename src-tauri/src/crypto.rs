use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use zeroize::Zeroizing;

/// Argon2id 参数配置
const MEMORY_COST: u32 = 65536; // 64 MB
const TIME_COST: u32 = 3;
const PARALLELISM: u32 = 4;
const OUTPUT_LENGTH: usize = 32;

/// 密码哈希结果
#[derive(Debug, Clone)]
pub struct HashedPassword {
    pub hash: String,
    pub salt: String,
}

/// 对密码进行 Argon2id 哈希
/// 使用 Zeroizing 包装密码，确保使用后内存自动清零（防止 dump 泄露）
pub fn hash_password(password: String) -> HashedPassword {
    // 使用 Zeroizing 包装，离开作用域时自动清零
    let password = Zeroizing::new(password);

    let salt = SaltString::generate(&mut OsRng);

    let argon2 = Argon2::new(
        argon2::Algorithm::Argon2id,
        argon2::Version::V0x13,
        argon2::Params::new(MEMORY_COST, TIME_COST, PARALLELISM, Some(OUTPUT_LENGTH))
            .expect("Invalid Argon2 parameters"),
    );

    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .expect("Failed to hash password");

    HashedPassword {
        hash: password_hash.to_string(),
        salt: salt.to_string(),
    }
    // password (Zeroizing) 离开作用域，内存自动清零
}

/// 验证密码
/// 使用 Zeroizing 包装密码，确保使用后内存自动清零（防止 dump 泄露）
pub fn verify_password(password: String, stored_hash: &str) -> bool {
    // 使用 Zeroizing 包装，离开作用域时自动清零
    let password = Zeroizing::new(password);

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
    // password (Zeroizing) 离开作用域，内存自动清零
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_and_verify() {
        let password = "test_password_123".to_string();
        let hashed = hash_password(password.clone());

        assert!(!hashed.hash.is_empty());
        assert!(!hashed.salt.is_empty());
        assert!(verify_password(password.clone(), &hashed.hash));
        assert!(!verify_password("wrong_password".to_string(), &hashed.hash));
    }

    #[test]
    fn test_different_salts() {
        let password = "same_password".to_string();
        let hash1 = hash_password(password.clone());
        let hash2 = hash_password(password);

        // 相同密码应产生不同的哈希值
        assert_ne!(hash1.hash, hash2.hash);
        assert_ne!(hash1.salt, hash2.salt);
    }
}
