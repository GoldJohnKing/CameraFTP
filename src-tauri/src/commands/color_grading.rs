// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::Arc;
use std::path::PathBuf;
use tauri::{command, State};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};

use crate::error::AppError;
use crate::color_grading::presets::{ColorGradingPreset, all_presets, METERING_MODES};
use crate::color_grading::service::ColorGradingService;

#[command]
pub async fn get_color_grading_presets() -> Vec<ColorGradingPreset> {
    all_presets().to_vec()
}

#[command]
pub fn get_metering_modes() -> Vec<(&'static str, &'static str)> {
    METERING_MODES.to_vec()
}

#[command]
pub async fn enqueue_color_grading(
    color_grading: State<'_, Arc<ColorGradingService>>,
    file_paths: Vec<String>,
    lut_id: String,
    metering_mode: String,
    ev_offset: f32,
) -> Result<(), AppError> {
    let paths: Vec<PathBuf> = file_paths.iter().map(PathBuf::from).collect();
    color_grading.enqueue(paths, lut_id, metering_mode, ev_offset).await
}

#[command]
pub async fn cancel_color_grading(
    color_grading: State<'_, Arc<ColorGradingService>>,
) -> Result<(), AppError> {
    color_grading.cancel();
    Ok(())
}

#[command]
pub fn is_raw_file(file_path: String) -> bool {
    crate::image_utils::is_raw_file(&PathBuf::from(file_path))
}

use crate::color_grading::preview::ColorGradingPreviewState;

#[command]
pub async fn begin_color_grading_preview(
    image_path: String,
    half_size: Option<bool>,
    max_preview_width: Option<u32>,
    max_preview_height: Option<u32>,
) -> Result<(), AppError> {
    let lensfun_db_path = crate::color_grading::resources::get_resources()
        .ok()
        .map(|r| r.lensfun_db_dir.to_string_lossy().into_owned());
    ColorGradingPreviewState::get_global()
        .begin(
            &image_path,
            lensfun_db_path.as_deref(),
            half_size.unwrap_or(false),
            max_preview_width.unwrap_or(0),
            max_preview_height.unwrap_or(0),
        )
        .await
}

#[command]
pub async fn apply_color_grading_preview(
    lut_id: String,
    metering_mode: String,
    ev_offset: f32,
    max_width: Option<u32>,
    max_height: Option<u32>,
) -> Result<String, AppError> {
    let mw = max_width.unwrap_or(0);
    let mh = max_height.unwrap_or(0);
    let jpeg_bytes = ColorGradingPreviewState::get_global()
        .apply(&lut_id, &metering_mode, ev_offset, mw, mh).await?;
    let b64 = BASE64.encode(&jpeg_bytes);
    Ok(format!("data:image/jpeg;base64,{}", b64))
}

#[command]
pub async fn end_color_grading_preview() -> Result<(), AppError> {
    ColorGradingPreviewState::get_global().end().await
}
