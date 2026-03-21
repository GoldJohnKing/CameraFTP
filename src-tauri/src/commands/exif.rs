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
        .map(|ratio| {
            let fstop = ratio.0 as f64 / ratio.1 as f64;
            format!("f/{:.1}", fstop)
        });

    // 提取快门速度 (1/125s 格式)
    let shutter_speed = exif.get(ExifTag::ExposureTime)
        .and_then(|v| v.as_urational())
        .map(|ratio| {
            let exposure = ratio.0 as f64 / ratio.1 as f64;
            if exposure < 1.0 && exposure > 0.0 {
                let denominator = ((1.0 / exposure).round() as u32).to_string();
                format!("1/{}s", denominator)
            } else {
                format!("{:.1}s", exposure)
            }
        });

    // 提取焦距，优先 35mm 等效焦距
    let focal_length = exif.get(ExifTag::FocalLengthIn35mmFilm)
        .and_then(|v| v.as_u16())
        .map(|v| format!("{}mm", v))
        .or_else(|| {
            exif.get(ExifTag::FocalLength)
                .and_then(|v| v.as_urational())
                .map(|ratio| {
                    let length = (ratio.0 as f64 / ratio.1 as f64).round() as u32;
                    format!("{}mm", length)
                })
        });

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