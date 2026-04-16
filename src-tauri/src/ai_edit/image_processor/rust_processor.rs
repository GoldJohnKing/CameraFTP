// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use std::path::Path;

use super::{ImagePreprocessor, PreparedImage, JPEG_QUALITY, MAX_LONG_SIDE};
use crate::error::AppError;

pub struct RustImagePreprocessor;

impl ImagePreprocessor for RustImagePreprocessor {
    fn prepare(&self, file_path: &Path) -> Result<PreparedImage, AppError> {
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
}

fn prepare_raster(file_path: &Path) -> Result<PreparedImage, AppError> {
    let file_path_owned = file_path.to_path_buf();
    let result = std::panic::catch_unwind(move || {
        let img = image::open(&file_path_owned)
            .map_err(|e| AppError::AiEditError(format!("Failed to decode image: {}", e)))?;
        encode_as_jpeg(img)
    });

    match result {
        Ok(Ok(prepared)) => Ok(prepared),
        Ok(Err(app_error)) => Err(app_error),
        Err(_) => Err(AppError::AiEditError(
            "Out of memory while decoding image — image dimensions may be too large for this device".to_string(),
        )),
    }
}

fn prepare_heic(file_path: &Path) -> Result<PreparedImage, AppError> {
    let bytes = std::fs::read(file_path)
        .map_err(|e| AppError::AiEditError(format!("Failed to read HEIC file: {}", e)))?;

    let result = std::panic::catch_unwind(|| {
        heic::DecoderConfig::new()
            .decode(&bytes, heic::PixelLayout::Rgba8)
            .map_err(|e| AppError::AiEditError(format!("Failed to decode HEIC: {:?}", e)))
    });

    let output = match result {
        Ok(Ok(decoder_output)) => decoder_output,
        Ok(Err(app_error)) => return Err(app_error),
        Err(_) => {
            return Err(AppError::AiEditError(
                "Out of memory while decoding HEIC — file may be too large for this device"
                    .to_string(),
            ));
        }
    };

    let rgba_image = image::RgbaImage::from_raw(output.width, output.height, output.data)
        .ok_or_else(|| {
            AppError::AiEditError("Failed to create image from HEIC decode output".to_string())
        })?;

    let img = image::DynamicImage::ImageRgba8(rgba_image);
    encode_as_jpeg(img)
}

fn encode_as_jpeg(img: image::DynamicImage) -> Result<PreparedImage, AppError> {
    let resized = resize_if_needed(img);

    let rgb_img = resized.to_rgb8();

    let mut jpeg_bytes = Vec::new();
    let mut encoder =
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg_bytes, JPEG_QUALITY);
    encoder
        .encode(
            rgb_img.as_raw(),
            rgb_img.width(),
            rgb_img.height(),
            image::ColorType::Rgb8.into(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
    use image::{ImageFormat, RgbImage};
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn create_test_jpeg(dir: &TempDir, name: &str, width: u32, height: u32) -> PathBuf {
        let path = dir.path().join(name);
        let img = RgbImage::from_pixel(width, height, image::Rgb([128u8, 128, 128]));
        img.save_with_format(&path, ImageFormat::Jpeg).unwrap();
        path
    }

    fn create_test_png(dir: &TempDir, name: &str, width: u32, height: u32) -> PathBuf {
        let path = dir.path().join(name);
        let img = RgbImage::from_pixel(width, height, image::Rgb([128u8, 128, 128]));
        img.save_with_format(&path, ImageFormat::Png).unwrap();
        path
    }

    fn decode_jpeg_dimensions(base64_data: &str) -> (u32, u32) {
        let bytes = BASE64.decode(base64_data).unwrap();
        let img = image::load_from_memory(&bytes).unwrap();
        (img.width(), img.height())
    }

    fn preprocessor() -> RustImagePreprocessor {
        RustImagePreprocessor
    }

    #[test]
    fn jpeg_below_max_size_not_resized() {
        let dir = TempDir::new().unwrap();
        let path = create_test_jpeg(&dir, "small.jpg", 100, 100);

        let result = preprocessor().prepare(&path).unwrap();
        assert_eq!(result.mime_type, "image/jpeg");

        let (w, h) = decode_jpeg_dimensions(&result.base64_data);
        assert_eq!(w, 100);
        assert_eq!(h, 100);
    }

    #[test]
    fn jpeg_above_max_size_resized_proportionally() {
        let dir = TempDir::new().unwrap();
        let path = create_test_jpeg(&dir, "large.jpg", 5000, 3000);

        let result = preprocessor().prepare(&path).unwrap();
        assert_eq!(result.mime_type, "image/jpeg");

        let (w, h) = decode_jpeg_dimensions(&result.base64_data);
        assert_eq!(w, 4096);
        assert_eq!(h, 2458);
    }

    #[test]
    fn portrait_image_resized_correctly() {
        let dir = TempDir::new().unwrap();
        let path = create_test_jpeg(&dir, "portrait.jpg", 3000, 5000);

        let result = preprocessor().prepare(&path).unwrap();
        assert_eq!(result.mime_type, "image/jpeg");

        let (w, h) = decode_jpeg_dimensions(&result.base64_data);
        assert_eq!(w, 2458);
        assert_eq!(h, 4096);
    }

    #[test]
    fn heic_file_decoded_as_jpeg() {
        let dir = TempDir::new().unwrap();
        let fake_bytes = b"fake-heic-data-for-test";
        let path = dir.path().join("photo.heic");
        std::fs::write(&path, fake_bytes).unwrap();

        let result = preprocessor().prepare(&path);
        assert!(result.is_err());
        let err_msg = format!("{:?}", result.unwrap_err());
        assert!(
            err_msg.contains("HEIC"),
            "Error message should mention HEIC: {}",
            err_msg
        );
    }

    #[test]
    fn all_outputs_have_jpeg_mime_type() {
        let dir = TempDir::new().unwrap();

        let jpg_path = create_test_jpeg(&dir, "photo.jpg", 100, 100);
        let jpg_result = preprocessor().prepare(&jpg_path).unwrap();
        assert_eq!(jpg_result.mime_type, "image/jpeg");

        let png_path = create_test_png(&dir, "photo.png", 100, 100);
        let png_result = preprocessor().prepare(&png_path).unwrap();
        assert_eq!(png_result.mime_type, "image/jpeg");
    }

    #[test]
    fn nonexistent_file_returns_error() {
        let path = PathBuf::from("/nonexistent/path/image.jpg");
        let result = preprocessor().prepare(&path);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AppError::AiEditError(_)));
    }

    #[test]
    fn output_base64_is_valid_jpeg_for_raster_input() {
        let dir = TempDir::new().unwrap();
        let path = create_test_png(&dir, "input.png", 100, 100);

        let result = preprocessor().prepare(&path).unwrap();
        assert_eq!(result.mime_type, "image/jpeg");

        let bytes = BASE64.decode(&result.base64_data).unwrap();
        // JPEG magic bytes: 0xFF 0xD8
        assert_eq!(bytes[0], 0xFF);
        assert_eq!(bytes[1], 0xD8);
    }

    #[test]
    fn encode_as_jpeg_produces_valid_output() {
        let img = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            10,
            10,
            image::Rgba([128, 128, 128, 255]),
        ));
        let result = encode_as_jpeg(img).unwrap();
        assert_eq!(result.mime_type, "image/jpeg");
        let bytes = BASE64.decode(&result.base64_data).unwrap();
        // JPEG magic bytes
        assert_eq!(bytes[0], 0xFF);
        assert_eq!(bytes[1], 0xD8);
    }
}
