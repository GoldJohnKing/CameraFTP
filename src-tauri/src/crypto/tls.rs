// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! FTPS 证书管理模块
//!
//! 负责自签名证书的生成、存储和轮换。

use rcgen::{generate_simple_self_signed, CertifiedKey};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// 证书文件名
const CERT_FILE: &str = "ftp.crt";
const KEY_FILE: &str = "ftp.key";
/// Companion file storing the cert generation timestamp (epoch seconds)
const CERT_TIMESTAMP_FILE: &str = "ftp.crt.generated";

/// 证书有效期：10年
const CERT_VALIDITY_DAYS: u64 = 365 * 10;

/// 过期前多少天开始轮换
const ROTATION_BUFFER_DAYS: u64 = 30;

/// 证书文件路径
#[derive(Debug, Clone)]
pub struct CertificatePaths {
    pub cert_path: PathBuf,
    pub key_path: PathBuf,
}

/// 确保证书有效，必要时生成新证书
///
/// # Returns
///
/// 返回证书和私钥的文件路径
///
/// # Errors
///
/// 当证书生成或文件写入失败时返回错误
pub fn ensure_valid_certificate() -> crate::error::AppResult<CertificatePaths> {
    let certs_dir = get_certs_directory()?;
    let cert_path = certs_dir.join(CERT_FILE);
    let key_path = certs_dir.join(KEY_FILE);

    let paths = CertificatePaths {
        cert_path: cert_path.clone(),
        key_path: key_path.clone(),
    };

    // 检查证书是否存在
    if !cert_path.exists() || !key_path.exists() {
        info!("TLS certificates not found, generating new ones");
        return generate_and_save_certificates(&cert_path, &key_path);
    }

    // 检查证书是否即将过期
    match check_certificate_validity(&cert_path) {
        Ok(days_until_expiry) => {
            if days_until_expiry < ROTATION_BUFFER_DAYS {
                info!(
                    days_until_expiry = days_until_expiry,
                    "TLS certificate expiring soon, rotating"
                );
                return generate_and_save_certificates(&cert_path, &key_path);
            }
            info!(
                days_until_expiry = days_until_expiry,
                "TLS certificates are valid"
            );
        }
        Err(e) => {
            warn!(error = %e, "Failed to check certificate validity, regenerating");
            return generate_and_save_certificates(&cert_path, &key_path);
        }
    }

    Ok(paths)
}

/// 获取证书存储目录
fn get_certs_directory() -> crate::error::AppResult<PathBuf> {
    let config_dir = crate::config::AppConfig::config_path()
        .parent()
        .ok_or_else(|| crate::error::AppError::Other("Invalid config path".to_string()))?
        .to_path_buf();

    let certs_dir = config_dir.join("certs");

    // 确保证书目录存在
    if !certs_dir.exists() {
        fs::create_dir_all(&certs_dir).map_err(|e| crate::error::AppError::Io(e.to_string()))?;
    }

    Ok(certs_dir)
}

/// 检查证书剩余有效期（天数）
fn check_certificate_validity(cert_path: &Path) -> Result<u64, Box<dyn std::error::Error>> {
    let timestamp_path = cert_path.with_extension("crt.generated");

    let created_epoch = if timestamp_path.exists() {
        fs::read_to_string(&timestamp_path)?
            .trim()
            .parse::<u64>()?
    } else {
        // Fallback for certs generated before this update
        let metadata = fs::metadata(cert_path)?;
        let created = metadata.created().or_else(|_| metadata.modified())?;
        created.duration_since(std::time::SystemTime::UNIX_EPOCH)?.as_secs()
    };

    let now_epoch = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)?
        .as_secs();

    let elapsed_days = (now_epoch - created_epoch) / 86400;
    let remaining = CERT_VALIDITY_DAYS.saturating_sub(elapsed_days);
    Ok(remaining)
}

/// 生成并保存自签名证书
fn generate_and_save_certificates(
    cert_path: &Path,
    key_path: &Path,
) -> crate::error::AppResult<CertificatePaths> {
    info!("Generating new self-signed TLS certificate");

    // 生成自签名证书
    let CertifiedKey { cert, key_pair } =
        generate_simple_self_signed(vec!["CameraFTP".to_string(), "localhost".to_string()])
            .map_err(|e| crate::error::AppError::Other(e.to_string()))?;

    let cert_pem = cert.pem();
    let key_pem = key_pair.serialize_pem();

    // 写入证书文件
    fs::write(cert_path, cert_pem).map_err(|e| crate::error::AppError::Io(e.to_string()))?;
    fs::write(key_path, key_pem).map_err(|e| crate::error::AppError::Io(e.to_string()))?;

    // Store generation timestamp for reliable validity tracking
    let timestamp_path = cert_path.with_extension("crt.generated");
    let now_epoch = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    fs::write(&timestamp_path, now_epoch.to_string())
        .map_err(|e| crate::error::AppError::Io(e.to_string()))?;

    // 设置文件权限（仅限 Unix）
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(key_path)
            .map_err(|e| crate::error::AppError::Io(e.to_string()))?
            .permissions();
        perms.set_mode(0o600);
        fs::set_permissions(key_path, perms)
            .map_err(|e| crate::error::AppError::Io(e.to_string()))?;
    }

    info!(
        cert_path = %cert_path.display(),
        key_path = %key_path.display(),
        "TLS certificates generated successfully"
    );

    Ok(CertificatePaths {
        cert_path: cert_path.to_path_buf(),
        key_path: key_path.to_path_buf(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn check_validity_returns_full_days_for_fresh_cert() {
        let dir = tempdir().expect("tempdir");
        let cert_path = dir.path().join("test.crt");
        let timestamp_path = dir.path().join("test.crt.generated");

        fs::write(&cert_path, "fake cert").expect("write cert");

        let now_epoch = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        fs::write(&timestamp_path, now_epoch.to_string()).expect("write timestamp");

        let remaining = check_certificate_validity(&cert_path).expect("check validity");
        assert_eq!(remaining, CERT_VALIDITY_DAYS);
    }

    #[test]
    fn check_validity_shows_low_days_for_old_cert() {
        let dir = tempdir().expect("tempdir");
        let cert_path = dir.path().join("test.crt");
        let timestamp_path = dir.path().join("test.crt.generated");

        fs::write(&cert_path, "fake cert").expect("write cert");

        // Simulate cert created 9 years ago
        let nine_years_ago = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            - (365 * 9 * 86400);
        fs::write(&timestamp_path, nine_years_ago.to_string()).expect("write timestamp");

        let remaining = check_certificate_validity(&cert_path).expect("check validity");
        // 10 year cert - 9 years elapsed = ~1 year remaining, less than 365+30 day buffer
        assert!(remaining < 400, "should be within rotation buffer: got {remaining}");
        assert!(remaining > 0, "should still have some days remaining");
    }

    #[test]
    fn check_validity_falls_back_to_file_mtime_without_timestamp() {
        let dir = tempdir().expect("tempdir");
        let cert_path = dir.path().join("test.crt");

        fs::write(&cert_path, "fake cert").expect("write cert");
        // No timestamp file — should fall back to file mtime

        let remaining = check_certificate_validity(&cert_path).expect("check validity");
        // Just created, should be near CERT_VALIDITY_DAYS
        assert!(remaining >= CERT_VALIDITY_DAYS - 1, "fresh cert should have near-full validity: got {remaining}");
    }
}
