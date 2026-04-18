// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use tauri::command;
use crate::error::AppError;

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
/// 使用 nom-exif 单库实现，支持 JPG/PNG/HEIF/RAW/CR3/NEF 等全格式
#[command]
pub async fn get_image_exif(file_path: String) -> Result<Option<ExifInfo>, AppError> {
    use nom_exif::*;

    let start = std::time::Instant::now();

    let mut parser = MediaParser::new();
    let ms = MediaSource::file_path(&file_path)
        .map_err(|e| AppError::Io(e.to_string()))?;

    // 检查是否有 EXIF 数据
    if !ms.has_exif() {
        tracing::debug!("No EXIF data found in {}", file_path);
        return Ok(None);
    }

    // 解析 EXIF
    let iter: ExifIter = match parser.parse(ms) {
        Ok(iter) => iter,
        Err(e) => {
            tracing::warn!("Failed to parse EXIF for {}: {:?}", file_path, e);
            return Ok(None);
        }
    };

    let exif: Exif = iter.into();

    // 提取 ISO
    let iso = exif.get(ExifTag::ISOSpeedRatings)
        .and_then(|v| v.as_u16())
        .map(|v| v as u32);

    // 提取光圈 (f/2.8 格式)
    let aperture = exif.get(ExifTag::FNumber)
        .and_then(|v| v.as_urational())
        .map(|ratio| format_aperture(ratio.0, ratio.1));

    // 提取快门速度 (1/125s 格式)
    let shutter_speed = exif.get(ExifTag::ExposureTime)
        .and_then(|v| v.as_urational())
        .map(|ratio| format_shutter_speed(ratio.0, ratio.1));

    // 提取焦距，优先 35mm 等效焦距
    let focal_35mm = exif.get(ExifTag::FocalLengthIn35mmFilm)
        .and_then(|v| v.as_u16());
    let focal_raw = exif.get(ExifTag::FocalLength)
        .and_then(|v| v.as_urational())
        .map(|ratio| (ratio.0, ratio.1));
    let focal_length = format_focal_length(focal_35mm, focal_raw);

    // 提取拍摄时间
    let datetime = exif.get(ExifTag::DateTimeOriginal)
        .and_then(|v| v.as_time_components())
        .map(|(ndt, _offset)| {
            ndt.format("%Y-%m-%d %H:%M:%S").to_string()
        });

    let duration = start.elapsed();
    tracing::debug!(
        "EXIF parsed for {} in {:?}: ISO={:?}, Aperture={:?}, Shutter={:?}, Focal={:?}, DateTime={:?}",
        file_path, duration, iso, aperture, shutter_speed, focal_length, datetime
    );

    // 如果没有有效数据，返回 None
    if iso.is_none() && aperture.is_none() && shutter_speed.is_none()
        && focal_length.is_none() && datetime.is_none() {
        return Ok(None);
    }

    Ok(Some(ExifInfo {
        iso,
        aperture,
        shutter_speed,
        focal_length,
        datetime,
    }))
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