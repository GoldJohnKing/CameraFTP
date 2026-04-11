// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! 文件系统工具模块
//!
//! 提供跨平台的文件系统辅助函数。

use std::path::Path;
use std::time::{Duration, Instant};
use tracing::{debug, trace};

/// 等待文件可读取（文件写入完成）
///
/// 通过轮询检查文件是否可打开读取，而非使用固定延迟。
/// 这比固定延迟更可靠，能适应不同大小的文件和不同的I/O速度。
///
/// # Arguments
/// * `path` - 文件路径
/// * `max_wait` - 最大等待时间
///
/// # Returns
/// * `true` - 文件已就绪（可读取）
/// * `false` - 超时仍未就绪
///
/// # Example
/// ```ignore
/// use std::time::Duration;
/// use camera_ftp_companion_lib::utils::fs::wait_for_file_ready;
///
/// if wait_for_file_ready(Path::new("/path/to/file.jpg"), Duration::from_secs(5)).await {
///     // 文件已就绪，可以安全读取
/// }
/// ```
pub async fn wait_for_file_ready(path: &Path, max_wait: Duration) -> bool {
    let start = Instant::now();
    let check_interval = Duration::from_millis(10);

    while start.elapsed() < max_wait {
        match is_file_readable(path).await {
            Ok(true) => {
                trace!(
                    "File ready after {:?}: {:?}",
                    start.elapsed(),
                    path
                );
                return true;
            }
            Ok(false) => {
                // 文件存在但可能还在写入，继续等待
                trace!("File not yet ready, waiting: {:?}", path);
            }
            Err(_) => {
                // 文件不存在或其他错误，继续等待（可能文件还没创建）
            }
        }
        tokio::time::sleep(check_interval).await;
    }

    debug!(
        "Timeout waiting for file ready after {:?}: {:?}",
        start.elapsed(),
        path
    );
    false
}

/// 检查文件是否可读取
///
/// 文件可读取意味着：
/// 1. 文件存在
/// 2. 可以成功打开（没有写入锁）
///
/// # Returns
/// * `Ok(true)` - 文件存在且可读取
/// * `Ok(false)` - 文件存在但可能被锁定
/// * `Err(_)` - 文件不存在或其他错误
async fn is_file_readable(path: &Path) -> Result<bool, std::io::Error> {
    // 检查文件是否存在
    if !tokio::fs::try_exists(path).await.unwrap_or(false) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "File does not exist",
        ));
    }

    // 尝试打开文件读取
    match tokio::fs::File::open(path).await {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(false),
        Err(e) => Err(e),
    }
}

/// 检查路径是否可写（通过创建临时测试文件）
///
/// 注意：不检查路径是否存在，直接尝试创建测试文件。
/// 如果路径不存在，创建操作会失败返回 false。
///
/// # Arguments
/// * `path` - 要检查的路径
///
/// # Returns
/// * `true` - 路径可写
/// * `false` - 路径不可写或不存在
///
/// # Example
/// ```ignore
/// use camera_ftp_companion_lib::utils::fs::is_path_writable;
/// use std::path::Path;
///
/// let writable = is_path_writable(Path::new("/tmp"));
/// ```
pub fn is_path_writable(path: &Path) -> bool {
    let test_file = path.join(".write_test");
    match std::fs::File::create(&test_file) {
        Ok(_) => {
            let _ = std::fs::remove_file(&test_file);
            true
        }
        Err(_) => false,
    }
}

