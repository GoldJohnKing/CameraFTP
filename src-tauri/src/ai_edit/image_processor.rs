// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use image::DynamicImage;
use std::path::Path;

use crate::error::AppError;

/// 图片长边最大像素（硬编码常量）
const MAX_LONG_SIDE: u32 = 4096;
/// JPEG 重编码质量
const JPEG_QUALITY: u8 = 85;

/// 读取图片文件，缩放至长边不超过 MAX_LONG_SIDE，重编码为 JPEG，返回 base64 字符串。
/// 无论原始格式（JPG/HEIF 等），输出始终为 JPEG。
pub fn prepare_for_upload(file_path: &Path) -> Result<String, AppError> {
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
            resized.color(),
        )
        .map_err(|e| AppError::AiEditError(format!("Failed to encode JPEG: {}", e)))?;

    Ok(BASE64.encode(&jpeg_bytes))
}

fn resize_if_needed(img: DynamicImage) -> DynamicImage {
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
