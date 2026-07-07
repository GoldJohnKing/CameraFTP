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

/// Decompressed NN demosaic ONNX model weights, handed to the C++ core via
/// `ra_set_nn_model` (Option D: ORT loads them from memory — no on-disk model
/// file, no extraction step, no upgrade-staleness class). Each is `None` when
/// the embedded payload is absent or failed to decompress (degrades cleanly to
/// classical demosaic).
///
/// The models are embedded at compile time (build.rs gzips them into
/// `OUT_DIR/nn_models/`). For the legacy variant build.rs writes empty gzip
/// placeholders so this still compiles; they decompress to empty → `None`.
pub fn nn_model_bytes() -> (Option<Vec<u8>>, Option<Vec<u8>>) {
    (
        decompress_nn_model(include_bytes!(concat!(
            env!("OUT_DIR"),
            "/nn_models/bayer.onnx.gz"
        ))),
        decompress_nn_model(include_bytes!(concat!(
            env!("OUT_DIR"),
            "/nn_models/xtrans.onnx.gz"
        ))),
    )
}

fn decompress_nn_model(compressed: &[u8]) -> Option<Vec<u8>> {
    use flate2::read::GzDecoder;
    use std::io::Read;
    let mut out = Vec::new();
    match GzDecoder::new(compressed).read_to_end(&mut out) {
        // Empty payload (legacy variant's placeholder gzip) → None → classical fallback.
        Ok(_) if !out.is_empty() => Some(out),
        _ => None,
    }
}

/// QNN context-cache dir (Android only at runtime — the C++ core only consumes
/// it on Android). This is the one NN artifact that still lives on disk: the
/// *compiled QNN graph* (context binary), distinct from the model weights (now
/// in-memory). Use filesDir (not cacheDir): Android may auto-clear cacheDir
/// under storage pressure, needlessly retriggering the ~5s graph compile.
#[cfg(target_os = "android")]
pub fn nn_ctx_dir(app_data_dir: &std::path::Path) -> Option<PathBuf> {
    let dir = app_data_dir.join("nn_ctx");
    match std::fs::create_dir_all(&dir) {
        Ok(()) => Some(dir),
        Err(e) => {
            tracing::warn!(error = %e, "could not create nn_ctx dir; QNN context cache disabled");
            None
        }
    }
}

/// Mapped QNN SoC config for the current device (Android only).
pub struct NnSocConfig {
    pub soc_model: &'static str,
    pub htp_arch: &'static str,
}

/// Resolve the device's Qualcomm SoC model to the QNN EP `soc_model` /
/// `htp_arch` options (Android only).
///
/// Reads Android system property `ro.soc.model` (= `Build.SOC_MODEL`), maps it
/// to the numeric value QNN EP's `soc_model` option expects, and returns both.
/// Consumed by the C++ core via `ra_set_nn_config` → `NnSessionConfig.socModel`.
/// Returns `None` when the property can't be read (QNN then auto-detects with
/// soc_model="0"). Replaces the former `configure_nn_soc_model_env` which set
/// `RA_NN_*` env vars — those were invisible to MSVC `std::getenv` on Windows
/// (CRT/Win32 desync).
#[cfg(target_os = "android")]
pub fn nn_soc_config() -> Option<NnSocConfig> {
    match read_build_soc_model() {
        Some(sm) => {
            let numeric = sm_to_qnn_soc_model(&sm);
            let arch = sm_to_qnn_htp_arch(&sm);
            tracing::info!(soc_model = %sm, qnn_numeric = %numeric, htp_arch = %arch, "QNN soc_model + htp_arch configured");
            Some(NnSocConfig { soc_model: numeric, htp_arch: arch })
        }
        None => {
            tracing::warn!("Could not read ro.soc.model; QNN will auto-detect (soc_model=0)");
            None
        }
    }
}

#[cfg(target_os = "android")]
fn read_build_soc_model() -> Option<String> {
    // Maximum length of a system property value (sys/system_properties.h,
    // PROP_VALUE_MAX). Hard-coded rather than imported from an NDK header.
    const PROP_VALUE_MAX: usize = 92;

    extern "C" {
        fn __system_property_get(
            name: *const std::os::raw::c_char,
            value: *mut std::os::raw::c_char,
        ) -> i32;
    }
    let mut buf = [0u8; PROP_VALUE_MAX];
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

// NN demosaic model loading is now in-memory (Option D): the embedded gzip
// payloads are decompressed to RAM by `nn_model_bytes` and handed to the C++
// core via `ra_set_nn_model`; the C++ core feeds them to ORT's in-memory
// `Ort::Session(env, ptr, len, opts)` ctor. There is no on-disk extraction, so
// there is no freshness/staleness surface here (the whole class — including the
// former hash-sidecar — is gone).
