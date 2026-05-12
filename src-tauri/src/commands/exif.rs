// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use tauri::command;
use crate::error::AppError;
use std::path::Path;

/// EXIF 信息结构体
#[derive(Debug, Clone, serde::Serialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ExifInfo {
    pub iso: Option<u32>,
    pub aperture: Option<String>,           // f/2.8 格式
    #[serde(rename = "shutterSpeed")]
    pub shutter_speed: Option<String>,      // 1/125s 格式
    #[serde(rename = "focalLength")]
    pub focal_length: Option<String>,       // 24mm 格式
    pub datetime: Option<String>,           // 2024-02-27 14:30:00 格式
    pub orientation: Option<u8>,            // EXIF Orientation (1-8)
}

/// Format shutter speed from an exposure time ratio.
pub(crate) fn format_shutter_speed(numerator: u32, denominator: u32) -> String {
    let exposure = numerator as f64 / denominator as f64;
    if exposure < 1.0 && exposure > 0.0 {
        let denom = (1.0 / exposure).round() as u32;
        format!("1/{}s", denom)
    } else {
        format!("{:.1}s", exposure)
    }
}

/// Format aperture from an f-number ratio.
pub(crate) fn format_aperture(numerator: u32, denominator: u32) -> String {
    let fstop = numerator as f64 / denominator as f64;
    format!("f/{:.1}", fstop)
}

/// Format focal length, preferring 35mm equivalent over raw.
pub(crate) fn format_focal_length(
    focal_35mm: Option<u16>,
    focal_raw: Option<(u32, u32)>,
) -> Option<String> {
    focal_35mm
        .map(|v| format!("{}mm", v))
        .or_else(|| {
            focal_raw.map(|(num, den)| {
                format!("{}mm", (num as f64 / den as f64).round() as u32)
            })
        })
}

/// 获取图片的 EXIF 信息
#[command]
pub async fn get_image_exif(file_path: String) -> Result<Option<ExifInfo>, AppError> {
    let start = std::time::Instant::now();

    let parsed = crate::image_utils::parse_exif(std::path::Path::new(&file_path))
        .map_err(|e| AppError::Io(e))?;

    let parsed = match parsed {
        Some(p) => p,
        None => {
            tracing::debug!("No EXIF data found in {}", file_path);
            return Ok(None);
        }
    };

    let iso = parsed.iso;
    let aperture = parsed.aperture.map(|r| format_aperture(r.0, r.1));
    let shutter_speed = parsed.shutter_speed.map(|r| format_shutter_speed(r.0, r.1));
    let focal_length = format_focal_length(
        parsed.focal_length_35mm,
        parsed.focal_length_raw.map(|r| (r.0, r.1)),
    );
    let datetime = parsed.datetime_original.map(|ndt| ndt.format("%Y-%m-%d %H:%M:%S").to_string());
    let orientation = parsed.orientation;

    let duration = start.elapsed();
    tracing::debug!(
        "EXIF parsed for {} in {:?}: ISO={:?}, Aperture={:?}, Shutter={:?}, Focal={:?}, DateTime={:?}, Orientation={:?}",
        file_path, duration, iso, aperture, shutter_speed, focal_length, datetime, orientation
    );

    if iso.is_none() && aperture.is_none() && shutter_speed.is_none()
        && focal_length.is_none() && datetime.is_none() && orientation.is_none() {
        return Ok(None);
    }

    Ok(Some(ExifInfo {
        iso,
        aperture,
        shutter_speed,
        focal_length,
        datetime,
        orientation,
    }))
}

/// Read the EXIF Orientation tag value from an image file.
/// Returns `0` if the file has no EXIF or no Orientation tag (valid values are 1–8).
#[command]
pub async fn get_raw_orientation(file_path: String) -> Result<u8, AppError> {
    let parsed = crate::image_utils::parse_exif(Path::new(&file_path))
        .map_err(|e| AppError::Io(e))?;
    Ok(parsed.and_then(|p| p.orientation).unwrap_or(0))
}

/// Inject EXIF orientation into a JPEG file that lacks it.
/// Reads orientation from a RAW source file via nomexif, then injects a minimal
/// APP1 segment into the JPEG at `thumbnail_path`.
/// Returns `true` if orientation was injected, `false` if not needed.
#[command]
pub async fn inject_exif_orientation(
    thumbnail_path: String,
    orientation: u8,
) -> Result<bool, AppError> {
    if orientation == 1 || orientation == 0 {
        return Ok(false);
    }

    let jpeg = std::fs::read(&thumbnail_path)
        .map_err(|e| AppError::Io(format!("Failed to read thumbnail: {}", e)))?;

    let fixed = crate::image_utils::inject_orientation_exif(jpeg, orientation);
    std::fs::write(&thumbnail_path, fixed)
        .map_err(|e| AppError::Io(format!("Failed to write thumbnail: {}", e)))?;

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fast_shutter_produces_fraction() {
        assert_eq!(format_shutter_speed(1, 250), "1/250s");
    }

    #[test]
    fn very_fast_shutter_rounds_correctly() {
        assert_eq!(format_shutter_speed(1, 4000), "1/4000s");
    }

    #[test]
    fn slow_shutter_produces_decimal() {
        assert_eq!(format_shutter_speed(5, 2), "2.5s");
    }

    #[test]
    fn one_second_shutter() {
        assert_eq!(format_shutter_speed(1, 1), "1.0s");
    }

    #[test]
    fn half_second_shutter() {
        assert_eq!(format_shutter_speed(1, 2), "1/2s");
    }

    #[test]
    fn aperture_formats_with_one_decimal() {
        assert_eq!(format_aperture(28, 10), "f/2.8");
    }

    #[test]
    fn aperture_integer_value() {
        assert_eq!(format_aperture(8, 1), "f/8.0");
    }

    #[test]
    fn prefers_35mm_equivalent() {
        let result = format_focal_length(Some(50), Some((35, 1)));
        assert_eq!(result, Some("50mm".to_string()));
    }

    #[test]
    fn falls_back_to_raw_when_no_35mm() {
        let result = format_focal_length(None, Some((23, 1)));
        assert_eq!(result, Some("23mm".to_string()));
    }

    #[test]
    fn raw_focal_rounds() {
        let result = format_focal_length(None, Some((5001, 100)));
        assert_eq!(result, Some("50mm".to_string()));
    }

    #[test]
    fn returns_none_when_both_missing() {
        let result = format_focal_length(None, None);
        assert_eq!(result, None);
    }

    #[test]
    fn ignores_raw_when_35mm_present() {
        let result = format_focal_length(Some(85), Some((24, 1)));
        assert_eq!(result, Some("85mm".to_string()));
    }
}