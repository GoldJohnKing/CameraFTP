// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

pub mod config;
pub mod progress;
pub(crate) mod image_processor;
pub(crate) mod providers;
pub(crate) mod service;

pub use config::AiEditConfig;
pub use service::AiEditService;
