// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod service;
mod types;

// 文件系统监听模块仅在 Windows 平台启用
#[cfg(target_os = "windows")]
pub mod watcher;

pub use service::FileIndexService;
pub use types::FileInfo;
