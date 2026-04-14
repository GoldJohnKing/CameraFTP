// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use std::path::Path;

use crate::error::AppError;

/// 图片长边最大像素（硬编码常量）
const MAX_LONG_SIDE: u32 = 4096;
/// JPEG 重编码质量
const JPEG_QUALITY: u8 = 85;

/// Result of preparing an image for upload to the AI edit API.
pub struct PreparedImage {
    /// Base64 encoded image data
    pub base64_data: String,
    /// MIME type for the data URI prefix (e.g., "image/jpeg" or "image/heic")
    pub mime_type: &'static str,
}

/// 读取图片文件，可选缩放并重编码为 JPEG。
/// HEIC/HEIF 文件直接发送原始字节（API 服务端解码）；JPG/PNG 文件在本地缩放后重编码为 JPEG。
pub fn prepare_for_upload(file_path: &Path) -> Result<PreparedImage, AppError> {
    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    if matches!(ext.as_str(), "heic" | "heif" | "hif") {
        return prepare_heic(file_path);
    }

    prepare_raster(file_path)
}

fn prepare_heic(file_path: &Path) -> Result<PreparedImage, AppError> {
    let bytes = std::fs::read(file_path)
        .map_err(|e| AppError::AiEditError(format!("Failed to read HEIC file: {}", e)))?;
    Ok(PreparedImage {
        base64_data: BASE64.encode(&bytes),
        mime_type: "image/heic",
    })
}

fn prepare_raster(file_path: &Path) -> Result<PreparedImage, AppError> {
    let img = image::open(file_path)
        .map_err(|e| AppError::AiEditError(format!("Failed to open image: {}", e)))?;

    let resized = resize_if_needed(img);

    let mut jpeg_bytes = Vec::new();
    let mut encoder =
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg_bytes, JPEG_QUALITY);
    encoder
        .encode(
            resized.as_bytes(),
            resized.width(),
            resized.height(),
            resized.color().into(),
        )
        .map_err(|e| AppError::AiEditError(format!("Failed to encode JPEG: {}", e)))?;

    Ok(PreparedImage {
        base64_data: BASE64.encode(&jpeg_bytes),
        mime_type: "image/jpeg",
    })
}

fn resize_if_needed(img: image::DynamicImage) -> image::DynamicImage {
    let (w, h) = (img.width(), img.height());
    let long_side = w.max(h);

    if long_side <= MAX_LONG_SIDE {
        return img;
    }

    let scale = MAX_LONG_SIDE as f64 / long_side as f64;
    let new_w = (w as f64 * scale).round() as u32;
    let new_h = (h as f64 * scale).round() as u32;
    img.resize_exact(new_w, new_h, image::imageops::FilterType::Lanczos3)
}
