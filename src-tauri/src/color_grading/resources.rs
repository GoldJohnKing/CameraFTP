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
/// `RA_NN_BAYER_MODEL` and `RA_NN_XTRANS_MODEL` at init time. The models are
/// extracted to `{app_data_dir}/models/` by `extract_nn_models()` at startup;
/// the `exists()` guard makes a failed extraction degrade cleanly to classical
/// demosaic rather than pointing at nonexistent paths. Safe to call before the
/// NN init.
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

/// Extract the embedded, gzip-compressed NN demosaic ONNX models to
/// `{app_data_dir}/models/` so the C++ NN core can load them via real
/// filesystem paths.
///
/// Tauri resources aren't auto-extracted to the data dir on Windows and aren't
/// accessible as paths at all on Android (APK assets), so we embed at compile
/// time (build.rs gzips the models into `OUT_DIR/nn_models/`) and extract
/// here — the same pattern as `lensfun_db::ensure_db()`. Idempotent: skips
/// files that already exist to avoid the multi-MB decompress+write on every
/// startup.
pub fn extract_nn_models(app_data_dir: &std::path::Path) -> Result<(), AppError> {
    let models_dir = app_data_dir.join("models");
    std::fs::create_dir_all(&models_dir)
        .map_err(|e| AppError::ColorGradingError(format!("Failed to create models dir: {}", e)))?;

    extract_one_nn_model(
        &models_dir,
        "bayer.onnx",
        include_bytes!(concat!(env!("OUT_DIR"), "/nn_models/bayer.onnx.gz")),
    )?;
    extract_one_nn_model(
        &models_dir,
        "xtrans.onnx",
        include_bytes!(concat!(env!("OUT_DIR"), "/nn_models/xtrans.onnx.gz")),
    )?;

    Ok(())
}

/// Decompress one embedded model to `models_dir/name`, skipping if it already
/// exists. `compressed` is the gzip payload written by build.rs.
fn extract_one_nn_model(
    models_dir: &std::path::Path,
    name: &str,
    compressed: &[u8],
) -> Result<(), AppError> {
    use flate2::read::GzDecoder;
    use std::io::Read;

    let out_path = models_dir.join(name);
    if out_path.exists() {
        return Ok(());
    }

    let mut decoder = GzDecoder::new(compressed);
    let mut data = Vec::new();
    decoder.read_to_end(&mut data).map_err(|e| {
        AppError::ColorGradingError(format!("Failed to decompress NN model '{}': {}", name, e))
    })?;
    std::fs::write(&out_path, &data).map_err(|e| {
        AppError::ColorGradingError(format!("Failed to write NN model '{}': {}", name, e))
    })?;
    tracing::info!(
        "NN model extracted: {} ({} KB)",
        out_path.display(),
        data.len() / 1024
    );
    Ok(())
}
