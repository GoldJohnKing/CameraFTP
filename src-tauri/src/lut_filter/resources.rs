// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::PathBuf;
use crate::error::AppError;

pub struct ResourcePaths {
    pub lensfun_db_dir: PathBuf,
    pub lut_presets_dir: PathBuf,
}

/// Ensure resource files (LUTs + Lensfun DB) are extracted to app data directory.
/// Returns paths to the extracted resource directories.
pub fn ensure_resources(_app_data_dir: &std::path::Path) -> Result<ResourcePaths, AppError> {
    // TODO: Implement resource extraction for bundled LUTs and Lensfun DB.
    Ok(ResourcePaths {
        lensfun_db_dir: _app_data_dir.join("lensfun_db"),
        lut_presets_dir: _app_data_dir.join("lut_presets"),
    })
}
