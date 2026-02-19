// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

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

/// 密码哈希结果（PHC 格式包含所有参数和盐值）
#[derive(Debug, Clone)]
pub struct HashedPassword {
    pub hash: String,
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
    }
    // password (Zeroizing) 离开作用域，内存自动清零
}

/// 验证密码
/// 使用 Zeroizing 包装密码，确保使用后内存自动清零（防止 dump 泄露）
/// 自动从哈希字符串中提取参数（PHC 格式），支持不同参数配置的哈希验证
pub fn verify_password(password: String, stored_hash: &str) -> bool {
    // 使用 Zeroizing 包装，离开作用域时自动清零
    let password = Zeroizing::new(password);

    let parsed_hash = match PasswordHash::new(stored_hash) {
        Ok(h) => h,
        Err(_) => return false,
    };

    // 使用 Argon2::default()，它会自动从 parsed_hash 中提取参数
    // 这样可以兼容不同参数生成的哈希（如未来升级参数时旧配置仍可验证）
    Argon2::default()
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
        // PHC 格式哈希包含 $ 分隔符
        assert!(hashed.hash.contains('$'));
        assert!(verify_password(password.clone(), &hashed.hash));
        assert!(!verify_password("wrong_password".to_string(), &hashed.hash));
    }

    #[test]
    fn test_different_salts() {
        let password = "same_password".to_string();
        let hash1 = hash_password(password.clone());
        let hash2 = hash_password(password);

        // 相同密码应产生不同的哈希值（因为盐值不同）
        assert_ne!(hash1.hash, hash2.hash);
    }
}
