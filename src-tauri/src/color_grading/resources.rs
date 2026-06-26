// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Color grading resource management.
//!
//! Delegates Lensfun DB extraction to the `lensfun_db` module which embeds
//! XML files at compile time and extracts them at runtime.

use std::path::PathBuf;
use std::sync::OnceLock;

use crate::error::AppError;

pub struct ResourcePaths {
    pub lensfun_db_dir: PathBuf,
}

static GLOBAL_RESOURCES: OnceLock<ResourcePaths> = OnceLock::new();

pub fn ensure_resources(
    app_data_dir: &std::path::Path,
) -> Result<(), AppError> {
    if GLOBAL_RESOURCES.get().is_some() {
        return Ok(());
    }

    // Extract embedded Lensfun DB to {app_data_dir}/lensfun_db/
    super::lensfun_db::ensure_db(app_data_dir)?;

    // Retrieve the extraction directory (set by ensure_db)
    let db = super::lensfun_db::get_db()?;

    let _ = GLOBAL_RESOURCES.set(ResourcePaths {
        lensfun_db_dir: db.db_dir.clone(),
    });

    tracing::info!("Color grading resources ready: lensfun={:?}", db.db_dir);
    Ok(())
}

pub fn get_resources() -> Result<&'static ResourcePaths, AppError> {
    GLOBAL_RESOURCES.get().ok_or_else(|| {
        AppError::ColorGradingError(
            "Resources not initialized. Call ensure_resources() first.".into(),
        )
    })
}

/// Point the C++ NN demosaic core at the on-disk model files via env vars.
///
/// The C++ side (`raDemosaicNnInit` / `decodeRawNn`) reads
/// `RA_NN_BAYER_MODEL` and `RA_NN_XTRANS_MODEL` at init time. We set them only
/// when the files actually exist in `{app_data_dir}/models/` so that, until
/// Task 7 packages the models, init degrades cleanly to classical demosaic
/// rather than pointing at nonexistent paths. Safe to call before the NN init.
pub fn configure_nn_model_env(app_data_dir: &std::path::Path) {
    let models_dir = app_data_dir.join("models");
    let bayer = models_dir.join("bayer.onnx");
    let xtrans = models_dir.join("xtrans.onnx");

    if bayer.exists() {
        std::env::set_var("RA_NN_BAYER_MODEL", &bayer);
        tracing::info!(path = %bayer.display(), "NN bayer model path configured");
    }
    if xtrans.exists() {
        std::env::set_var("RA_NN_XTRANS_MODEL", &xtrans);
        tracing::info!(path = %xtrans.display(), "NN xtrans model path configured");
    }
}
