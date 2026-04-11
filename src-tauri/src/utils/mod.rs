// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! 工具模块
//!
//! 提供跨平台的通用辅助函数和 trait。

pub mod fs;

// 公开常用函数以便直接使用
pub use fs::{is_path_writable, wait_for_file_ready};
