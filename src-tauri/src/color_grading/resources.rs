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

    // QNN context-cache dir (Android only at runtime, but setting the env vars
    // on all platforms is harmless — the C++ core only consumes them on Android).
    // Use filesDir (not cacheDir): Android may auto-clear cacheDir under storage
    // pressure, which would needlessly retrigger the ~5s graph compile.
    #[cfg(target_os = "android")]
    {
        let nn_ctx_dir = app_data_dir.join("nn_ctx");
        if let Err(e) = std::fs::create_dir_all(&nn_ctx_dir) {
            tracing::warn!(error = %e, "could not create nn_ctx dir; QNN context cache disabled");
        } else {
            std::env::set_var("RA_NN_CTX_DIR", &nn_ctx_dir);
        }
    }
    std::env::set_var("RA_NN_APP_VERSION", env!("CARGO_PKG_VERSION"));
}

/// Publish the device's Qualcomm SoC model to the C++ NN core via env var.
///
/// Reads Android system property `ro.soc.model` (= `Build.SOC_MODEL`), maps it
/// to the numeric value QNN EP's `soc_model` option expects, and sets
/// `RA_NN_SOC_MODEL`. Read by `raw_alchemy_capi.cpp` into `DecodeParams.nnSocModel`,
/// then consumed by `nn_session.cpp` QNN EP options. "0" (unknown) lets QNN auto-detect.
#[cfg(target_os = "android")]
pub fn configure_nn_soc_model_env() {
    match read_build_soc_model() {
        Some(sm) => {
            let numeric = sm_to_qnn_soc_model(&sm);
            let arch = sm_to_qnn_htp_arch(&sm);
            std::env::set_var("RA_NN_SOC_MODEL", numeric);
            std::env::set_var("RA_NN_HTP_ARCH", arch);
            tracing::info!(soc_model = %sm, qnn_numeric = %numeric, htp_arch = %arch, "QNN soc_model + htp_arch configured");
        }
        None => {
            tracing::warn!("Could not read ro.soc.model; QNN will auto-detect (soc_model=0)");
        }
    }
}

#[cfg(target_os = "android")]
fn read_build_soc_model() -> Option<String> {
    extern "C" {
        fn __system_property_get(
            name: *const std::os::raw::c_char,
            value: *mut std::os::raw::c_char,
        ) -> i32;
    }
    let mut buf = [0u8; 92]; // PROP_VALUE_MAX
    let n = unsafe {
        __system_property_get(
            b"ro.soc.model\0".as_ptr() as *const _,
            buf.as_mut_ptr() as *mut _,
        )
    };
    if n > 0 {
        Some(String::from_utf8_lossy(&buf[..n as usize]).into_owned())
    } else {
        None
    }
}

/// Maps `Build.SOC_MODEL` → QNN `soc_model` numeric string (from QNN SDK's Qnn_SocModel_t).
/// Verified against ORT 1.24.1 + ExecuTorch qc_schema.py + LiteRT supported_soc.csv.
#[cfg(target_os = "android")]
fn sm_to_qnn_soc_model(sm: &str) -> &'static str {
    match sm {
        "SM8550" => "43", // Hexagon v73
        "SM8650" => "57", // Hexagon v75
        "SM8750" => "69", // Hexagon v79
        "SM8845" => "97", // Hexagon v81
        "SM8850" => "87", // Hexagon v81
        _ => "0",         // unknown → QNN auto-detect
    }
}

/// Maps `Build.SOC_MODEL` → QNN HTP architecture string. Empty = let QNN infer
/// from soc_model (used when the SoC is outside the known whitelist).
#[cfg(target_os = "android")]
fn sm_to_qnn_htp_arch(sm: &str) -> &'static str {
    match sm {
        "SM8550" => "73",
        "SM8650" => "75",
        "SM8750" => "79",
        "SM8845" | "SM8850" => "81",
        _ => "",
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
