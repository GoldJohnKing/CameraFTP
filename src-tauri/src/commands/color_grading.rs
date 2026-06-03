// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::PathBuf;
use tauri::{command, State};

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
    color_grading: State<'_, ColorGradingService>,
    file_paths: Vec<String>,
    lut_id: String,
    use_auto_exposure: bool,
    metering_mode: String,
    manual_ev: f32,
) -> Result<(), AppError> {
    let paths: Vec<PathBuf> = file_paths.iter().map(PathBuf::from).collect();
    color_grading.enqueue(paths, lut_id, use_auto_exposure, metering_mode, manual_ev).await
}

#[command]
pub async fn cancel_color_grading(
    color_grading: State<'_, ColorGradingService>,
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
    preview: State<'_, ColorGradingPreviewState>,
    image_path: String,
) -> Result<(), AppError> {
    let lensfun_db_path = crate::color_grading::resources::get_resources()
        .ok()
        .map(|r| r.lensfun_db_dir.to_string_lossy().into_owned());
    let path: Option<&str> = lensfun_db_path.as_deref();
    preview.begin(&image_path, path).await
}

#[command]
pub async fn apply_color_grading_preview(
    preview: State<'_, ColorGradingPreviewState>,
    lut_id: String,
    enable_lens_correction: bool,
    use_auto_exposure: bool,
    metering_mode: String,
    manual_ev: f32,
) -> Result<String, AppError> {
    preview.apply(&lut_id, enable_lens_correction, use_auto_exposure, &metering_mode, manual_ev).await
}

#[command]
pub async fn end_color_grading_preview(
    preview: State<'_, ColorGradingPreviewState>,
) -> Result<(), AppError> {
    preview.end().await
}
