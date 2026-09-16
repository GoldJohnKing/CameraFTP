// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! 文件系统工具模块
//!
//! 提供跨平台的文件系统辅助函数。

use std::path::Path;
use std::time::{Duration, Instant, SystemTime};
use tracing::{debug, trace};

/// (大小, mtime) 保持不变的持续时长，超过即视为写入完成。
/// 取 200ms：足以覆盖连续写入的分段间隙，又不明显拖慢单个文件的索引时机。
pub(crate) const FILE_READY_STABLE_WINDOW: Duration = Duration::from_millis(200);

/// 等待文件写入完成（大小与修改时间稳定）
///
/// 通过轮询 metadata 的 (len, mtime) 签名判断写入是否结束。
/// 为什么不用 File::open 成功与否判断：Windows 上 std 默认以
/// FILE_SHARE_READ|WRITE|DELETE 打开，写方持有句柄时 open 仍然成功；
/// notify 8.x 的 Windows 后端（ReadDirectoryChangesW）也不派发 Close
/// 事件，因此"稳定性探测"是唯一可靠的写完信号。
pub async fn wait_for_file_ready(path: &Path, max_wait: Duration) -> bool {
    let start = Instant::now();
    let poll_interval = Duration::from_millis(20);

    let mut last_sig: Option<(u64, Option<SystemTime>)> = None;
    let mut last_change = Instant::now();

    while start.elapsed() < max_wait {
        match tokio::fs::metadata(path).await {
            Ok(md) => {
                let sig = (md.len(), md.modified().ok());
                if Some(sig) == last_sig {
                    if last_change.elapsed() >= FILE_READY_STABLE_WINDOW {
                        trace!(
                            "File stable after {:?}: {:?}",
                            start.elapsed(),
                            path
                        );
                        return true;
                    }
                } else {
                    last_sig = Some(sig);
                    last_change = Instant::now();
                }
            }
            Err(_) => {
                // 文件尚未创建（或刚被删除）：重置稳定性基准
                last_sig = None;
                last_change = Instant::now();
            }
        }
        tokio::time::sleep(poll_interval).await;
    }

    debug!(
        "Timeout waiting for file ready after {:?}: {:?}",
        start.elapsed(),
        path
    );
    false
}

/// 检查路径是否可写（通过创建临时测试文件）
///
/// 直接在目标目录下创建临时测试文件。成功则删除并返回 `Ok(true)`；
/// 失败（路径不存在、权限不足等）返回 `Err(io::Error)`。
///
/// # Arguments
/// * `path` - 要检查的目录路径
///
/// # Returns
/// * `Ok(true)` - 路径可写
/// * `Err(io::Error)` - 路径不可写或不存在
///
/// # Example
/// ```ignore
/// use camera_ftp_companion_lib::utils::fs::is_path_writable;
/// use std::path::Path;
///
/// let writable = is_path_writable(Path::new("/tmp"));
/// ```
pub fn is_path_writable(path: &Path) -> Result<bool, std::io::Error> {
    let test_file = path.join(".write_test");
    match std::fs::File::create(&test_file) {
        Ok(_) => {
            let _ = std::fs::remove_file(&test_file);
            Ok(true)
        }
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[tokio::test]
    async fn wait_for_file_ready_returns_true_for_readable_file() {
        let mut file = tempfile::NamedTempFile::new().expect("create temp file");
        file.write_all(b"test content").expect("write content");
        file.flush().expect("flush");
        let path = file.path().to_path_buf();

        let result = wait_for_file_ready(&path, Duration::from_secs(2)).await;
        assert!(result, "file should be ready immediately");
    }

    #[tokio::test]
    async fn wait_for_file_ready_waits_for_stability_window() {
        let mut file = tempfile::NamedTempFile::new().expect("create temp file");
        file.write_all(b"test content").expect("write content");
        file.flush().expect("flush");
        let path = file.path().to_path_buf();

        let start = Instant::now();
        let result = wait_for_file_ready(&path, Duration::from_secs(2)).await;
        assert!(result, "stable file should become ready");
        // 不能"open 成功即返回"：必须等到稳定性窗口结束
        assert!(
            start.elapsed() >= FILE_READY_STABLE_WINDOW,
            "ready only after the stability window, took {:?}",
            start.elapsed()
        );
    }

    #[tokio::test]
    async fn wait_for_file_ready_not_ready_while_file_keeps_growing() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let path = temp_dir.path().join("growing.jpg");
        std::fs::write(&path, b"head").expect("initial write");

        // 模拟持续写入：每 40ms 追加一次共 ~600ms（间隔远小于稳定窗口）
        let writer_path = path.clone();
        let writer = tokio::spawn(async move {
            for i in 0..15u32 {
                tokio::time::sleep(Duration::from_millis(40)).await;
                use std::io::Write;
                let mut f = std::fs::OpenOptions::new()
                    .append(true)
                    .open(&writer_path)
                    .expect("open append");
                f.write_all(format!("-chunk{}", i).as_bytes())
                    .expect("append chunk");
            }
        });

        let start = Instant::now();
        let result = wait_for_file_ready(&path, Duration::from_secs(5)).await;
        let elapsed = start.elapsed();
        writer.await.expect("writer task finishes");

        assert!(result, "file eventually becomes stable");
        assert!(
            elapsed >= Duration::from_millis(600),
            "must not report ready while file is growing (took {:?})",
            elapsed
        );
    }

    #[tokio::test]
    async fn wait_for_file_ready_returns_false_for_nonexistent_file() {
        let path = std::env::temp_dir().join("nonexistent_test_file_12345_unique.jpg");
        let _ = std::fs::remove_file(&path);

        let result = wait_for_file_ready(&path, Duration::from_millis(100)).await;
        assert!(!result, "should timeout for nonexistent file");
    }

    #[test]
    fn is_path_writable_detects_writable_dir() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let result = is_path_writable(temp_dir.path()).expect("check writable");
        assert!(result);
        let test_file = temp_dir.path().join(".write_test");
        assert!(!test_file.exists(), "test file should be cleaned up");
    }

    #[test]
    fn is_path_writable_returns_error_for_nonexistent_dir() {
        let result = is_path_writable(Path::new("/nonexistent/path/that/does/not/exist/abc123"));
        assert!(result.is_err(), "should fail for nonexistent directory");
    }
}

