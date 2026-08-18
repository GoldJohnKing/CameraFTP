// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! RawAlchemy / 调色启动引导：从 `lib.rs` 的 Tauri setup 闭包整体迁出。
//!
//! 职责：资源释放、核心库（DLL/SO）路径解析与预加载、NN 权重与运行时
//! 配置注入、后台预热，以及 `ColorGradingService` 的构建与注册。

use tauri::Manager;

use crate::color_grading;
use crate::config_service::ConfigService;

/// C++ NN diagnostics log-file path, captured at logging setup and pushed into
/// the C++ core via `ra_set_log_file` after the RawAlchemy DLL loads (replaces
/// the former `RA_NN_LOG_FILE` env var, which was invisible to MSVC
/// `std::getenv` on Windows due to CRT/Win32 environment desync).
pub(crate) static NN_LOG_FILE: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();

/// 调色 / NN 引导入口，由 `lib.rs` 的 setup 闭包调用一次。
pub fn init(app: &tauri::AppHandle, config_service: &std::sync::Arc<ConfigService>) {
    let app_data_dir = app.path().app_data_dir()
        .expect("Failed to resolve app data dir");
    if let Err(e) = color_grading::resources::ensure_resources(&app_data_dir) {
        tracing::warn!("Color grading resource extraction failed: {}", e);
    }

    let resolved = resolve_raw_alchemy_lib_path();
    match color_grading::ffi::RawAlchemyLib::load_global(&resolved.lib_path) {
        Ok(lib) => {
            // Redirect C++ NN diagnostics into app.log (replaces the
            // former RA_NN_LOG_FILE env var — invisible to MSVC
            // std::getenv on Windows due to CRT/Win32 desync).
            if let Some(p) = NN_LOG_FILE.get().and_then(|p| p.to_str()) {
                lib.set_log_file(Some(p));
            }

            // Inject NN model weights + runtime config explicitly via the
            // C-ABI transport (Option D: weights go in-memory via
            // ra_set_nn_model — ORT loads them from a RAM buffer, no on-disk
            // model file; the rest via ra_set_nn_config). The C side
            // deep-copies both, so the caller's buffers may be freed right
            // after. MUST run before the warmup / lazy init below.
            let (bayer_bytes, xtrans_bytes) = color_grading::resources::nn_model_bytes();
            if let Some(b) = &bayer_bytes {
                if let Err(e) = lib.set_nn_model(0, b) {
                    tracing::error!("ra_set_nn_model(bayer) failed: {}", e);
                }
            }
            if let Some(x) = &xtrans_bytes {
                if let Err(e) = lib.set_nn_model(1, x) {
                    tracing::error!("ra_set_nn_model(xtrans) failed: {}", e);
                }
            }

            {
                use color_grading::ffi::RaNnConfig;

                // Leak a CString into a *const c_char. The C side
                // deep-copies, so the allocation is intentionally
                // never reclaimed (a handful of short strings, once).
                let cstr = |s: &str| -> *const std::os::raw::c_char {
                    match std::ffi::CString::new(s) {
                        Ok(c) => c.into_raw() as *const std::os::raw::c_char,
                        Err(e) => {
                            tracing::warn!(
                                "NN config string {:?} had interior NUL ({}); passing as NULL",
                                s, e
                            );
                            std::ptr::null()
                        }
                    }
                };

                let mut cfg = RaNnConfig::default();
                cfg.app_version = cstr(env!("CARGO_PKG_VERSION"));

                // QNN context-cache dir (Android only) — the one NN artifact
                // still on disk (the compiled graph), distinct from the
                // in-memory weights.
                #[cfg(target_os = "android")]
                if let Some(p) = color_grading::resources::nn_ctx_dir(&app_data_dir)
                    .as_ref()
                    .and_then(|p| p.to_str())
                {
                    cfg.ctx_dir = cstr(p);
                }

                // DirectML.dll path (Windows only) — extracted +
                // preloaded above; its parent dir steers ORT's DML
                // EP via SetDllDirectoryA in the C++ core.
                #[cfg(target_os = "windows")]
                if let Some(p) = resolved.directml_path.as_ref().and_then(|p| p.to_str()) {
                    cfg.directml_dll_path = cstr(p);
                }

                // QNN SoC params (Android only) — SM8550→"43"/"73"
                // mapping lives in resources::nn_soc_config.
                #[cfg(target_os = "android")]
                if let Some(s) = color_grading::resources::nn_soc_config() {
                    cfg.soc_model = cstr(s.soc_model);
                    cfg.htp_arch = cstr(s.htp_arch);
                }

                if let Err(e) = lib.set_nn_config(&cfg) {
                    tracing::error!("ra_set_nn_config failed: {}", e);
                }
            }

            // Log whether models were embedded. (Android → logcat via the
            // android_logging subscriber; desktop → app.log.)
            tracing::info!(
                "NN models: bayer={} xtrans={}",
                bayer_bytes.is_some(),
                xtrans_bytes.is_some()
            );
            // Eagerly compile/warm the NN session on a background thread on ALL
            // platforms (Android QNN ~2s, Windows DirectML ~hundreds of ms, Linux
            // CPU EP). Keeps the UI thread off the compile critical path; the first
            // edit falls back to classical via the router if it arrives before
            // warmup finishes. Thread-safe by the C++ init() mutex; the lazy
            // raw_decoder fallback is a last-resort safety net. NN_INIT_DONE guards
            // the router against a spurious "not ready" latch during this window.
            tauri::async_runtime::spawn_blocking(|| {
                color_grading::ffi::warmup_nn_session();
                // Init attempted (success or failure): the router may latch
                // structural unavailability from here on.
                color_grading::service::NN_INIT_DONE.store(
                    true,
                    std::sync::atomic::Ordering::Release,
                );
            });
        }
        Err(e) => tracing::error!("Failed to load RawAlchemyCpp: {}", e),
    }

    let cg_service = std::sync::Arc::new(color_grading::ColorGradingService::new(
        app.clone(),
        std::sync::Arc::clone(&config_service),
    ));
    app.manage(cg_service.clone());
    cg_service.set_global();
    // nn_enabled stays at its optimistic default (true) on all platforms:
    // the background warmup (above) may still succeed, and the router's
    // NN_INIT_DONE guard prevents a spurious latch while it's in flight; a
    // real structural failure latches nn_enabled=false on the first edit.
    color_grading::preview::ColorGradingPreviewState::ensure_init();
}

/// Resolved RawAlchemy core library path plus the extracted DirectML.dll path
/// (Windows only; `None` elsewhere or when the preload failed). The DirectML
/// path is handed to the C++ core via `ra_set_nn_config` so nn_session.cpp can
/// `SetDllDirectoryA` its parent dir.
struct ResolvedLibPath {
    lib_path: std::path::PathBuf,
    // Only read under #[cfg(target_os = "windows")] above; allow(dead_code) silences
    // the unused-field warning on Android/Linux where DirectML doesn't apply.
    #[allow(dead_code)]
    directml_path: Option<std::path::PathBuf>,
}

fn resolve_raw_alchemy_lib_path() -> ResolvedLibPath {
    #[cfg(target_os = "android")]
    {
        // Kotlin side calls System.loadLibrary("raw_alchemy_core") in MainActivity.onCreate().
        // After that, dlopen("libraw_alchemy_core.so") finds the already-loaded library.
        ResolvedLibPath {
            lib_path: std::path::PathBuf::from("libraw_alchemy_core.so"),
            directml_path: None,
        }
    }
    #[cfg(target_os = "windows")]
    {
        // DLL is embedded in the exe via include_bytes! and extracted to temp at startup.
        match color_grading::ffi::embedded_dll::extract_to_temp() {
            Ok(path) => {
                // Preload raw_alchemy_core.dll's name-resolved dependencies BEFORE
                // the caller LoadLibrary's the core DLL. Order matters:
                //   1. libomp.dll      — raw_alchemy_core imports it at load time
                //   2. DirectML.dll    — onnxruntime's DirectML EP loads it at runtime
                //   3. onnxruntime.dll — raw_alchemy_core imports it at load time
                // Each is extracted under its exact base name + leaked resident so
                // the core DLL's import-table resolution binds to these copies
                // (LoadLibraryEx flags=0 does not search the DLL's own directory).
                preload_or_log(color_grading::ffi::embedded_dll::preload_libomp, "libomp");
                // NN runtime DLLs (DirectML + onnxruntime) are only embedded by
                // the neural variant; preload_nn_runtime is a no-op for legacy.
                let directml_path = color_grading::ffi::embedded_dll::preload_nn_runtime();
                ResolvedLibPath { lib_path: path, directml_path }
            }
            Err(e) => {
                tracing::error!("Failed to extract embedded DLL: {}. Falling back to exe dir.", e);
                let exe_dir = std::env::current_exe()
                    .ok()
                    .and_then(|p| p.parent().map(|d| d.to_path_buf()))
                    .unwrap_or_else(|| std::path::PathBuf::from("."));
                ResolvedLibPath {
                    lib_path: exe_dir.join("raw_alchemy_core.dll"),
                    directml_path: None,
                }
            }
        }
    }
}

/// Run a DLL preload step, logging (not propagating) failures so one missing
/// dependency doesn't abort the whole preload sequence — the subsequent
/// raw_alchemy_core.dll load surfaces the real failure if a dependency is absent.
#[cfg(target_os = "windows")]
fn preload_or_log(preload: fn() -> Result<(), crate::error::AppError>, label: &str) {
    if let Err(e) = preload() {
        tracing::error!(
            "Failed to preload {}: {}. raw_alchemy_core.dll may fail to load.",
            label,
            e
        );
    }
}
