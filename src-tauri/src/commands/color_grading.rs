// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::PathBuf;
use tauri::{command, State};

use crate::error::AppError;
use crate::color_grading::presets::{ColorGradingPreset, all_presets};
use crate::color_grading::service::ColorGradingService;
use crate::image_utils;

#[command]
pub async fn get_color_grading_presets() -> Vec<ColorGradingPreset> {
    all_presets().to_vec()
}

#[command]
pub async fn enqueue_color_grading(
    color_grading: State<'_, ColorGradingService>,
    file_paths: Vec<String>,
    lut_id: String,
) -> Result<(), AppError> {
    let paths: Vec<PathBuf> = file_paths.iter().map(PathBuf::from).collect();
    color_grading.enqueue(paths, lut_id).await
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
    image_utils::is_raw_file(&PathBuf::from(file_path))
}
