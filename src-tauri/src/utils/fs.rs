// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! 文件系统工具模块
//!
//! 提供跨平台的文件系统辅助函数。

use std::path::Path;

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

/// 确保目录存在，如果不存在则创建
///
/// # Arguments
/// * `path` - 目录路径
///
/// # Returns
/// * `Ok(())` - 目录已存在或创建成功
/// * `Err(String)` - 创建失败，包含错误信息
pub fn ensure_dir_exists(path: &Path) -> Result<(), String> {
    if !path.exists() {
        std::fs::create_dir_all(path)
            .map_err(|e| format!("无法创建目录 '{}': {}", path.display(), e))?;
    }
    Ok(())
}
