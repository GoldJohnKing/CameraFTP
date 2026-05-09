// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::PathBuf;
use tauri::{command, State};

use crate::error::AppError;
use crate::lut_filter::presets::{PresetLut, all_presets};
use crate::lut_filter::service::LutFilterService;

#[command]
pub async fn get_preset_luts() -> Vec<PresetLut> {
    all_presets().to_vec()
}

#[command]
pub async fn enqueue_lut_filter(
    lut_filter: State<'_, LutFilterService>,
    file_paths: Vec<String>,
    lut_id: String,
) -> Result<(), AppError> {
    let paths: Vec<PathBuf> = file_paths.iter().map(PathBuf::from).collect();
    lut_filter.enqueue(paths, lut_id).await
}

#[command]
pub async fn cancel_lut_filter(
    lut_filter: State<'_, LutFilterService>,
) -> Result<(), AppError> {
    lut_filter.cancel();
    Ok(())
}

#[command]
pub fn is_raw_file(file_path: String) -> bool {
    let ext = PathBuf::from(&file_path)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    matches!(ext.as_str(),
        "nef" | "nrw" | "cr2" | "cr3" | "arw" | "sr2" | "raf" |
        "orf" | "rw2" | "pef" | "dng" | "x3f" | "raw" | "srw"
    )
}
