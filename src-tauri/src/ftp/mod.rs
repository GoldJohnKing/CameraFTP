// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! FTP服务器模块
//!
//! 该模块提供了完整的FTP服务器功能，采用Actor模式实现，包括：
//! - 事件驱动架构（EventBus）
//! - 统计信息Actor（StatsActor）
//! - 服务器Actor（FtpServerActor）
//! - 监听器（Listeners）
//!
//! ## 存储后端
//!
//! - Windows: 使用 `unftp_sbe_fs::Filesystem` 存储到本地文件系统
//! - Android: 使用 `android_mediastore::AndroidMediaStoreBackend` 存储到 MediaStore

pub mod android_mediastore;
pub mod events;
pub mod listeners;
pub mod server;
pub mod server_factory;
pub mod stats;
pub mod types;

// 平台特定的存储后端类型别名
#[cfg(target_os = "android")]
pub type FtpStorageBackend = crate::ftp::android_mediastore::AndroidMediaStoreBackend;

#[cfg(not(target_os = "android"))]
pub type FtpStorageBackend = unftp_sbe_fs::Filesystem;

// 重新导出主要类型
pub use events::EventBus;
pub(crate) use events::{EventProcessor, FrontendTransientEventHandler, StatsEventHandler, TrayUpdateHandler};
pub use server::{create_ftp_server, FtpServerHandle};
pub use types::{FtpAuthConfig, ServerInfo, ServerStateSnapshot};
