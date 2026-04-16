// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::error::AppError;
use std::path::Path;

#[allow(dead_code)]
pub const MAX_LONG_SIDE: u32 = 4096;
#[allow(dead_code)]
pub const JPEG_QUALITY: u8 = 85;

#[derive(Debug)]
pub struct PreparedImage {
    pub base64_data: String,
    pub mime_type: &'static str,
}

pub trait ImagePreprocessor: Send + Sync {
    fn prepare(&self, file_path: &Path) -> Result<PreparedImage, AppError>;
}

#[cfg(not(target_os = "android"))]
mod rust_processor;

#[cfg(not(target_os = "android"))]
pub fn create_preprocessor() -> Box<dyn ImagePreprocessor> {
    Box::new(rust_processor::RustImagePreprocessor)
}

#[cfg(target_os = "android")]
mod android_processor;

#[cfg(target_os = "android")]
pub fn create_preprocessor() -> Box<dyn ImagePreprocessor> {
    Box::new(android_processor::AndroidImagePreprocessor)
}
