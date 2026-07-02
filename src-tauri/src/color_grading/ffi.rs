// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::os::raw::{c_char, c_float, c_int};
use std::path::Path;
use std::sync::{Arc, OnceLock};

use libloading::Library;

use crate::error::AppError;

const DEFAULT_JPEG_QUALITY: c_int = 95;
const ENABLE_LENS_CORRECTION: c_int = 1;

#[cfg(target_os = "windows")]
pub mod embedded_dll {
    use super::*;

    const RAW_ALCHEMY_DLL_GZ: &[u8] =
        include_bytes!(concat!(env!("OUT_DIR"), "/raw_alchemy_core.dll.gz"));

    /// Extract the embedded gzip-compressed DLL to a temp directory.
    /// Uses a content hash in the filename so new versions replace old ones automatically.
    pub fn extract_to_temp() -> Result<std::path::PathBuf, AppError> {
        use std::hash::{Hash, Hasher};
        use std::io::Read;

        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        RAW_ALCHEMY_DLL_GZ.hash(&mut hasher);
        let content_hash = format!("{:016x}", hasher.finish());

        let temp_dir = std::env::temp_dir().join("CameraFTP");
        std::fs::create_dir_all(&temp_dir).map_err(|e| {
            AppError::ColorGradingError(format!("Failed to create temp dir {}: {}", temp_dir.display(), e))
        })?;

        let dll_name = format!("raw_alchemy_core_{}.dll", content_hash);
        let dll_path = temp_dir.join(&dll_name);

        if dll_path.exists() {
            tracing::debug!("Embedded DLL already extracted: {}", dll_path.display());
            cleanup_old_dlls(&temp_dir, "raw_alchemy_core_", &dll_name);
            return Ok(dll_path);
        }

        tracing::info!("Extracting embedded DLL to {}", dll_path.display());

        let mut decoder = flate2::read::GzDecoder::new(RAW_ALCHEMY_DLL_GZ);
        let mut dll_bytes = Vec::new();
        decoder.read_to_end(&mut dll_bytes).map_err(|e| {
            AppError::ColorGradingError(format!("Failed to decompress embedded DLL: {}", e))
        })?;

        if dll_bytes.is_empty() {
            return Err(AppError::ColorGradingError(
                "Embedded DLL is empty — RawAlchemyCpp was not built".into(),
            ));
        }

        // Write atomically: write to temp file then rename
        let tmp_path = dll_path.with_extension("tmp");
        std::fs::write(&tmp_path, &dll_bytes).map_err(|e| {
            AppError::ColorGradingError(format!("Failed to write DLL to {}: {}", tmp_path.display(), e))
        })?;
        std::fs::rename(&tmp_path, &dll_path).map_err(|e| {
            AppError::ColorGradingError(format!("Failed to rename DLL: {}", e))
        })?;

        cleanup_old_dlls(&temp_dir, "raw_alchemy_core_", &dll_name);

        Ok(dll_path)
    }

    /// Remove old versions of an extracted DLL from the temp directory.
    /// Matches files named `<prefix>*.dll` other than `current_name`. Used to GC
    /// both `raw_alchemy_core_<hash>.dll` and `libomp_<hash>.dll` across updates.
    fn cleanup_old_dlls(temp_dir: &Path, prefix: &str, current_name: &str) {
        if let Ok(entries) = std::fs::read_dir(temp_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.starts_with(prefix)
                    && name_str.ends_with(".dll")
                    && name_str != current_name
                {
                    if let Err(e) = std::fs::remove_file(entry.path()) {
                        tracing::debug!("Failed to remove old DLL {}: {}", name_str, e);
                    }
                }
            }
        }
    }

    // --- Embedded dependency DLLs (libomp, onnxruntime, DirectML) ---
    //
    // These three are load-time/runtime DEPENDENCIES of raw_alchemy_core.dll and
    // the ORT DirectML EP — they are resolved by NAME, not by the explicit path
    // the host uses for raw_alchemy_core.dll itself. libloading 0.8 loads with
    // LoadLibraryExW(flags=0), which does NOT search the loaded DLL's own
    // directory, so the dependencies must be findable as already-loaded modules
    // keyed by their EXACT base name (libomp.dll, onnxruntime.dll, DirectML.dll).
    //
    // They are therefore extracted under those exact names (NOT content-hashed
    // filenames — a `<name>_<hash>.dll` preload would NOT match a `<name>.dll`
    // import). Freshness across app updates is handled by a sidecar `<name>.hash`
    // file: when the embedded content hash changes, the DLL is overwritten in
    // place and the sidecar updated, giving the same staleness guarantee the
    // hashed-filename approach gives raw_alchemy_core.dll.
    const LIBOMP_DLL_GZ: &[u8] =
        include_bytes!(concat!(env!("OUT_DIR"), "/libomp.dll.gz"));
    #[cfg(nn_demosaic)]
    const ONNXRUNTIME_DLL_GZ: &[u8] =
        include_bytes!(concat!(env!("OUT_DIR"), "/onnxruntime.dll.gz"));
    #[cfg(nn_demosaic)]
    const DIRECTML_DLL_GZ: &[u8] =
        include_bytes!(concat!(env!("OUT_DIR"), "/directml.dll.gz"));

    /// Extract an embedded gzip DLL to the CameraFTP temp dir under `exact_name`,
    /// skipping the write when the sidecar content hash matches the embedded
    /// payload (so a 17 MB ORT DLL is not rewritten on every launch). Returns the
    /// extracted path. Atomic: writes to a `.tmp` sibling then renames.
    fn extract_dll_by_name(
        dll_gz: &[u8],
        exact_name: &str,
    ) -> Result<std::path::PathBuf, AppError> {
        use std::hash::{Hash, Hasher};
        use std::io::Read;

        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        dll_gz.hash(&mut hasher);
        let content_hash = format!("{:016x}", hasher.finish());

        let temp_dir = std::env::temp_dir().join("CameraFTP");
        std::fs::create_dir_all(&temp_dir).map_err(|e| {
            AppError::ColorGradingError(format!(
                "Failed to create temp dir {}: {}",
                temp_dir.display(),
                e
            ))
        })?;

        let dll_path = temp_dir.join(exact_name);
        let hash_path = temp_dir.join(format!("{}.hash", exact_name));

        // Freshness short-circuit: same hash sidecar + file present ⇒ on-disk copy
        // matches the embedded payload, skip the decompress+write.
        let up_to_date = dll_path.exists()
            && std::fs::read_to_string(&hash_path)
                .map(|h| h.trim() == content_hash)
                .unwrap_or(false);
        if up_to_date {
            return Ok(dll_path);
        }

        let mut decoder = flate2::read::GzDecoder::new(dll_gz);
        let mut bytes = Vec::new();
        decoder.read_to_end(&mut bytes).map_err(|e| {
            AppError::ColorGradingError(format!("Failed to decompress embedded {}: {}", exact_name, e))
        })?;

        // Empty payload means the DLL wasn't fetched/built for this profile
        // (e.g. a Debug cargo build without a matching RawAlchemyCpp build, or
        // the nn-cache wasn't populated). Fail with an actionable message
        // instead of writing a 0-byte DLL.
        if bytes.is_empty() {
            return Err(AppError::ColorGradingError(format!(
                "Embedded {} is empty — the dependency was not built/fetched for this profile",
                exact_name
            )));
        }

        // Atomic write: temp file + rename, then persist the hash sidecar.
        let tmp_path = dll_path.with_extension("tmp");
        std::fs::write(&tmp_path, &bytes).map_err(|e| {
            AppError::ColorGradingError(format!(
                "Failed to write {} to {}: {}",
                exact_name,
                tmp_path.display(),
                e
            ))
        })?;
        std::fs::rename(&tmp_path, &dll_path).map_err(|e| {
            AppError::ColorGradingError(format!("Failed to rename {}: {}", exact_name, e))
        })?;
        std::fs::write(&hash_path, &content_hash).map_err(|e| {
            AppError::ColorGradingError(format!("Failed to write {} hash sidecar: {}", exact_name, e))
        })?;

        Ok(dll_path)
    }

    /// Extract + LoadLibrary + leak the handle so the module stays resident for
    /// the process lifetime, registered under its exact base name. That makes a
    /// later name-based resolution (a `raw_alchemy_core.dll` import of
    /// `onnxruntime.dll`, or ORT's internal `LoadLibrary("DirectML.dll")`) bind
    /// to this already-loaded module — the only resolution path that works under
    /// LoadLibraryEx(flags=0) without the DLL being on PATH/in System32.
    fn preload_dll_by_name(
        dll_gz: &[u8],
        exact_name: &str,
    ) -> Result<std::path::PathBuf, AppError> {
        let dll_path = extract_dll_by_name(dll_gz, exact_name)?;
        let lib = unsafe { Library::new(&dll_path) }.map_err(|e| {
            AppError::ColorGradingError(format!(
                "Failed to preload {} from {}: {}",
                exact_name,
                dll_path.display(),
                e
            ))
        })?;
        // Leak: dropping could FreeLibrary the module while a dependent DLL still
        // references it. These runtimes are needed for the whole process lifetime.
        std::mem::forget(lib);
        tracing::debug!("Preloaded {} from {}", exact_name, dll_path.display());
        Ok(dll_path)
    }

    /// Preload libomp.dll (OpenMP runtime). `raw_alchemy_core.dll` imports it at
    /// load time, so it must be resident — by exact name — before the core DLL
    /// is LoadLibrary'd. Also sweeps legacy `libomp_<hash>.dll` files left by the
    /// previous content-hashed extraction scheme.
    pub fn preload_libomp() -> Result<(), AppError> {
        let temp_dir = std::env::temp_dir().join("CameraFTP");
        preload_dll_by_name(LIBOMP_DLL_GZ, "libomp.dll")?;
        // One-time sweep of legacy hashed filenames so they don't accumulate.
        cleanup_old_dlls(&temp_dir, "libomp_", "libomp.dll");
        Ok(())
    }

    /// Preload onnxruntime.dll (the DirectML-capable ORT build).
    /// `raw_alchemy_core.dll` links the ORT import lib, so it has a load-time
    /// dependency on onnxruntime.dll that must resolve to our embedded copy.
    #[cfg(nn_demosaic)]
    pub fn preload_onnxruntime() -> Result<(), AppError> {
        preload_dll_by_name(ONNXRUNTIME_DLL_GZ, "onnxruntime.dll")?;
        Ok(())
    }

    /// Preload DirectML.dll and return its path. ORT's DirectML EP loads
    /// DirectML.dll at runtime (DMLCreateDevice); the exact-name preload makes
    /// that bind to our copy. The returned path is handed to the C++ core via
    /// `ra_set_nn_config` (directml_dll_path) so nn_session.cpp can call
    /// `SetDllDirectoryA` on its parent dir as defense-in-depth against a stale
    /// System32 DirectML.dll (ORT issue #18831).
    #[cfg(nn_demosaic)]
    pub fn preload_directml() -> Result<std::path::PathBuf, AppError> {
        let dll_path = preload_dll_by_name(DIRECTML_DLL_GZ, "DirectML.dll")?;
        tracing::debug!(path = %dll_path.display(), "DirectML preloaded");
        Ok(dll_path)
    }

    /// Preload the NN runtime DLLs (DirectML + onnxruntime). Only the neural
    /// variant embeds them and has a C++ core that imports onnxruntime.dll; the
    /// legacy variant's C++ core is built without `RA_ENABLE_NN_DEMOSAIC`, so it
    /// has no ORT import dependency and build.rs embeds nothing to preload.
    /// Returns the DirectML path (handed to the C++ core via ra_set_nn_config)
    /// when present; None for the legacy variant or when preload failed.
    #[cfg(nn_demosaic)]
    pub fn preload_nn_runtime() -> Option<std::path::PathBuf> {
        let directml_path = match preload_directml() {
            Ok(p) => Some(p),
            Err(e) => {
                tracing::error!(
                    "Failed to preload DirectML: {}. raw_alchemy_core.dll may fail to load.",
                    e
                );
                None
            }
        };
        if let Err(e) = preload_onnxruntime() {
            tracing::error!(
                "Failed to preload onnxruntime: {}. raw_alchemy_core.dll may fail to load.",
                e
            );
        }
        directml_path
    }

    #[cfg(not(nn_demosaic))]
    pub fn preload_nn_runtime() -> Option<std::path::PathBuf> {
        None
    }
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RaResult {
    Ok = 0,
    ErrUnknown = -1,
    ErrFileNotFound = -2,
    ErrDecodeFailed = -3,
    ErrInvalidParam = -4,
    ErrLogUnsupported = -5,
    ErrLutLoadFailed = -6,
    ErrWriteFailed = -7,
    ErrNoLensProfile = -8,
    ErrOutOfMemory = -9,
    ErrNnNotInitialized = -10,
    ErrNnNanOutput = -11,
    ErrNnInferenceFailed = -12,
}

impl RaResult {
    pub fn is_ok(self) -> bool {
        self == Self::Ok
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::Ok => "Success",
            Self::ErrUnknown => "Unknown error",
            Self::ErrFileNotFound => "File not found",
            Self::ErrDecodeFailed => "RAW decode failed",
            Self::ErrInvalidParam => "Invalid parameter",
            Self::ErrLogUnsupported => "Log space unsupported",
            Self::ErrLutLoadFailed => "LUT load failed",
            Self::ErrWriteFailed => "Write failed",
            Self::ErrNoLensProfile => "No lens profile found",
            Self::ErrOutOfMemory => "Out of memory",
            Self::ErrNnNotInitialized => "NN demosaic session not initialized",
            Self::ErrNnNanOutput => "NN demosaic produced NaN/Inf output",
            Self::ErrNnInferenceFailed => "NN demosaic inference failed",
        }
    }
}

type RaProcessFileWithLUTFn = unsafe extern "C" fn(
    *const c_char,   // inputPath
    *const c_char,   // outputPath
    *const c_char,   // logSpace
    *const c_float,  // lutTable
    c_int,           // lutSize
    *const c_float,  // lutDomainMin
    *const c_float,  // lutDomainMax
    *const c_char,   // metering
    c_float,         // evOffset
    c_int,           // jpegQuality
    c_int,           // enableLensCorrection
    *const c_char,   // customLensfunDb
    c_int,           // enableNnDemosaic
) -> c_int;

type RaGetLastErrorFn = unsafe extern "C" fn() -> *const c_char;
type RaGetVersionFn = unsafe extern "C" fn() -> *const c_char;

/// Opaque handle to a C++ preview session (decoded RAW cached in C++ heap).
#[repr(transparent)]
pub(crate) struct RaPreviewSession {
    pub(crate) ptr: *mut std::ffi::c_void,
}

// SAFETY: RaPreviewSession is Send because all access to `ptr` is serialized by
// the async Mutex in ColorGradingPreviewState. The mutex guard is held across
// spawn_blocking in apply(), preventing concurrent begin/end from freeing the
// session while grading is in progress. JNI threads calling end() go through
// state.end() which also acquires the same mutex.
unsafe impl Send for RaPreviewSession {}

type RaBeginPreviewSessionFn = unsafe extern "C" fn(
    *const c_char,   // inputPath
    c_int,           // enableLensCorrection
    *const c_char,   // customLensfunDb
    c_int,           // halfSize
    c_int,           // maxPreviewWidth
    c_int,           // maxPreviewHeight
    *mut RaPreviewSession, // outSession
) -> c_int;

type RaApplyPreviewGradingFn = unsafe extern "C" fn(
    *mut std::ffi::c_void, // session
    *const c_char,         // logSpace
    *const c_float,        // lutTable
    c_int,                 // lutSize
    *const c_float,        // lutDomainMin
    *const c_float,        // lutDomainMax
    *const c_char,         // metering
    c_float,               // evOffset
    c_int,                 // jpegQuality
    c_int,                 // maxWidth
    c_int,                 // maxHeight
    *mut *mut u8,          // outBuffer
    *mut c_int,            // outLen
) -> c_int;

type RaEndPreviewSessionFn = unsafe extern "C" fn(
    *mut std::ffi::c_void, // session (RaPreviewSession.ptr)
);

type RaFreePreviewBufferFn = unsafe extern "C" fn(
    *mut u8, // buffer
);

// No args; reads the config set via ra_set_nn_config and drives
// NnDemosaicSession::init(). Always present in NN-enabled builds; resolved
// optionally so a build without the symbol still loads (warmup just no-ops
// with a debug log).
type RaWarmupNnSessionFn = unsafe extern "C" fn();

// True iff the NN demosaic session successfully initialized (NPU engaged).
// Optional: resolved like the other NN symbols so a build without it still
// loads — `is_nn_ready()` then returns false, steering the router to classical.
type RaIsNnReadyFn = unsafe extern "C" fn() -> bool;

// Explicit NN config transport — replaces the RA_NN_* env vars (which are
// invisible to MSVC std::getenv on Windows due to CRT/Win32 environment
// desync, so all NN config read as NULL there). Field order MUST match the C
// struct in raw_alchemy_capi.h exactly: directml, soc_model, htp_arch, ctx_dir,
// app_version. (Model WEIGHTS are carried separately via ra_set_nn_model —
// Option D: in-memory ONNX bytes, not file paths.)
#[repr(C)]
pub struct RaNnConfig {
    pub directml_dll_path: *const c_char,
    pub soc_model: *const c_char,
    pub htp_arch: *const c_char,
    pub ctx_dir: *const c_char,
    pub app_version: *const c_char,
}

impl Default for RaNnConfig {
    fn default() -> Self {
        Self {
            directml_dll_path: std::ptr::null(),
            soc_model: std::ptr::null(),
            htp_arch: std::ptr::null(),
            ctx_dir: std::ptr::null(),
            app_version: std::ptr::null(),
        }
    }
}

type RaSetNnConfigFn = unsafe extern "C" fn(*const RaNnConfig) -> c_int;
type RaSetLogFileFn = unsafe extern "C" fn(*const c_char);

// ra_set_nn_model supplies an NN model's ONNX weights as an in-memory byte
// buffer (Option D: ORT loads from memory, no on-disk file). kind: 0=bayer,
// 1=xtrans. The C side deep-copies, so the caller's buffer may be freed
// immediately. Optional like the other NN symbols.
type RaSetNnModelFn = unsafe extern "C" fn(
    kind: c_int,
    data: *const std::ffi::c_void,
    len: usize,
) -> c_int;

pub struct RawAlchemyLib {
    _lib: Library,
    process_file_with_lut: RaProcessFileWithLUTFn,
    get_last_error: RaGetLastErrorFn,
    get_version: RaGetVersionFn,
    begin_preview_session: RaBeginPreviewSessionFn,
    apply_preview_grading: RaApplyPreviewGradingFn,
    end_preview_session: RaEndPreviewSessionFn,
    free_preview_buffer: RaFreePreviewBufferFn,
    warmup_nn_session: Option<RaWarmupNnSessionFn>,
    is_nn_ready: Option<RaIsNnReadyFn>,
    set_nn_config: Option<RaSetNnConfigFn>,
    set_nn_model: Option<RaSetNnModelFn>,
    set_log_file: Option<RaSetLogFileFn>,
}

fn ra_result_from_code(code: c_int) -> RaResult {
    match code {
        0 => RaResult::Ok,
        -1 => RaResult::ErrUnknown,
        -2 => RaResult::ErrFileNotFound,
        -3 => RaResult::ErrDecodeFailed,
        -4 => RaResult::ErrInvalidParam,
        -5 => RaResult::ErrLogUnsupported,
        -6 => RaResult::ErrLutLoadFailed,
        -7 => RaResult::ErrWriteFailed,
        -8 => RaResult::ErrNoLensProfile,
        -9 => RaResult::ErrOutOfMemory,
        -10 => RaResult::ErrNnNotInitialized,
        -11 => RaResult::ErrNnNanOutput,
        -12 => RaResult::ErrNnInferenceFailed,
        _ => RaResult::ErrUnknown,
    }
}

static GLOBAL_LIB: OnceLock<Arc<RawAlchemyLib>> = OnceLock::new();

/// RAII guard that ensures the C++ preview buffer is freed on drop,
/// even during a panic unwind (e.g. OOM in to_vec()).
struct CppBufferGuard<'a> {
    buf: *mut u8,
    lib: &'a RawAlchemyLib,
}

impl<'a> Drop for CppBufferGuard<'a> {
    fn drop(&mut self) {
        if !self.buf.is_null() {
            unsafe { (self.lib.free_preview_buffer)(self.buf); }
        }
    }
}

impl RawAlchemyLib {
    pub fn load(path: &Path) -> Result<Self, AppError> {
        let lib = unsafe {
            Library::new(path).map_err(|e| {
                AppError::ColorGradingError(format!("Failed to load {}: {}", path.display(), e))
            })?
        };

        let process_file_with_lut = unsafe {
            *lib.get::<RaProcessFileWithLUTFn>(b"raProcessFileWithLUT\0")
                .map_err(|e| {
                    AppError::ColorGradingError(format!(
                        "Symbol raProcessFileWithLUT not found: {}",
                        e
                    ))
                })?
        };
        let get_last_error = unsafe {
            *lib.get::<RaGetLastErrorFn>(b"raGetLastError\0")
                .map_err(|e| {
                    AppError::ColorGradingError(format!("Symbol raGetLastError not found: {}", e))
                })?
        };
        let get_version = unsafe {
            *lib.get::<RaGetVersionFn>(b"raGetVersion\0")
                .map_err(|e| {
                    AppError::ColorGradingError(format!("Symbol raGetVersion not found: {}", e))
                })?
        };
        let begin_preview_session = unsafe {
            *lib.get::<RaBeginPreviewSessionFn>(b"raBeginPreviewSession\0")
                .map_err(|e| {
                    AppError::ColorGradingError(format!(
                        "Symbol raBeginPreviewSession not found: {}",
                        e
                    ))
                })?
        };
        let apply_preview_grading = unsafe {
            *lib.get::<RaApplyPreviewGradingFn>(b"raApplyPreviewGrading\0")
                .map_err(|e| {
                    AppError::ColorGradingError(format!(
                        "Symbol raApplyPreviewGrading not found: {}",
                        e
                    ))
                })?
        };
        let end_preview_session = unsafe {
            *lib.get::<RaEndPreviewSessionFn>(b"raEndPreviewSession\0")
                .map_err(|e| {
                    AppError::ColorGradingError(format!(
                        "Symbol raEndPreviewSession not found: {}",
                        e
                    ))
                })?
        };

        let free_preview_buffer = unsafe {
            *lib.get::<RaFreePreviewBufferFn>(b"raFreePreviewBuffer\0")
                .map_err(|e| {
                    AppError::ColorGradingError(format!("Symbol raFreePreviewBuffer not found: {}", e))
                })?
        };

        // raWarmupNnSession is the background-warmup entry. Optional: resolved so
        // a build without the symbol still loads (warmup then no-ops with a log).
        let warmup_nn_session = unsafe {
            lib.get::<RaWarmupNnSessionFn>(b"raWarmupNnSession\0")
                .ok()
                .map(|f| *f)
        };
        if warmup_nn_session.is_some() {
            tracing::debug!("raWarmupNnSession symbol resolved");
        } else {
            tracing::debug!("raWarmupNnSession symbol not present — background NN warmup disabled");
        }

        // raIsNnReady lets the router tell structural NN unavailability (NPU
        // not engaged → latch classical for the session) from a per-file NN
        // error on a ready session (→ retry this file classically, no latch).
        // Optional for the same robustness reason as the other NN symbols;
        // when absent, is_nn_ready() returns false → classical fallback.
        let is_nn_ready = unsafe {
            lib.get::<RaIsNnReadyFn>(b"raIsNnReady\0")
                .ok()
                .map(|f| *f)
        };
        if is_nn_ready.is_some() {
            tracing::debug!("raIsNnReady symbol resolved");
        } else {
            tracing::debug!("raIsNnReady symbol not present — router will treat NN as never ready");
        }

        // ra_set_nn_config is the explicit C-ABI NN config transport that
        // replaces the RA_NN_* env vars (env vars set by the Rust host are
        // invisible to MSVC std::getenv on Windows — CRT/Win32 desync — so
        // all NN config read as NULL there, defeating NN init). Optional for
        // the same robustness reason as the other NN symbols; when absent,
        // set_nn_config() returns an error and callers fall back to classical.
        let set_nn_config = unsafe {
            lib.get::<RaSetNnConfigFn>(b"ra_set_nn_config\0")
                .ok()
                .map(|f| *f)
        };
        if set_nn_config.is_some() {
            tracing::debug!("ra_set_nn_config symbol resolved");
        } else {
            tracing::warn!("ra_set_nn_config symbol not present — NN config injection disabled (NN demosaic will be unavailable)");
        }

        // ra_set_nn_model carries NN model weights as in-memory ONNX bytes
        // (Option D). Optional for the same robustness reason as the other NN
        // symbols; when absent, set_nn_model() returns an error → classical
        // fallback (no NN).
        let set_nn_model = unsafe {
            lib.get::<RaSetNnModelFn>(b"ra_set_nn_model\0")
                .ok()
                .map(|f| *f)
        };
        if set_nn_model.is_some() {
            tracing::debug!("ra_set_nn_model symbol resolved");
        } else {
            tracing::warn!("ra_set_nn_model symbol not present — NN model bytes cannot be injected (NN demosaic will be unavailable)");
        }

        // ra_set_log_file redirects C++ NN diagnostics (nnlog::info) into the
        // app log file. Optional; when absent, set_log_file() no-ops with a
        // debug log and the C++ side keeps writing to stderr.
        let set_log_file = unsafe {
            lib.get::<RaSetLogFileFn>(b"ra_set_log_file\0")
                .ok()
                .map(|f| *f)
        };
        if set_log_file.is_some() {
            tracing::debug!("ra_set_log_file symbol resolved");
        } else {
            tracing::debug!("ra_set_log_file symbol not present — C++ NN diagnostics stay on stderr");
        }

        Ok(Self {
            _lib: lib,
            process_file_with_lut,
            get_last_error,
            get_version,
            begin_preview_session,
            apply_preview_grading,
            end_preview_session,
            free_preview_buffer,
            warmup_nn_session,
            is_nn_ready,
            set_nn_config,
            set_nn_model,
            set_log_file,
        })
    }

    pub fn get() -> Result<&'static Arc<RawAlchemyLib>, AppError> {
        GLOBAL_LIB.get().ok_or_else(|| {
            AppError::ColorGradingError(
                "RawAlchemyCpp library not loaded. Call load_global() first.".into(),
            )
        })
    }

    pub fn load_global(path: &Path) -> Result<&'static Arc<RawAlchemyLib>, AppError> {
        if let Some(lib) = GLOBAL_LIB.get() {
            return Ok(lib);
        }
        let lib = Self::load(path)?;
        let version = lib.version();
        tracing::info!("RawAlchemyCpp loaded, version: {}", version);
        let _ = GLOBAL_LIB.set(Arc::new(lib));
        Ok(GLOBAL_LIB.get().unwrap())
    }

    pub fn version(&self) -> String {
        unsafe {
            let ptr = (self.get_version)();
            if ptr.is_null() {
                return "unknown".into();
            }
            std::ffi::CStr::from_ptr(ptr).to_string_lossy().into_owned()
        }
    }

    fn format_last_error(&self, ra_result: RaResult, raw_code: c_int) -> AppError {
        let last_error = unsafe {
            let ptr = (self.get_last_error)();
            if ptr.is_null() {
                String::new()
            } else {
                std::ffi::CStr::from_ptr(ptr).to_string_lossy().into_owned()
            }
        };
        AppError::ColorGradingError(if last_error.is_empty() {
            format!("{} ({})", ra_result.description(), raw_code)
        } else {
            format!("{}: {}", ra_result.description(), last_error)
        })
    }

    pub fn process_file_with_lut(
        &self,
        input_path: &Path,
        output_path: &Path,
        log_space: Option<&str>,
        lut_data: &Arc<super::lut_data::LutData>,
        lensfun_db_path: Option<&str>,
        ev_offset: f32,
        metering_mode: &str,
        enable_nn_demosaic: bool,
    ) -> Result<(), AppError> {
        let input_c = std::ffi::CString::new(input_path.to_string_lossy().into_owned())
            .map_err(|e| AppError::ColorGradingError(format!("Invalid input path: {}", e)))?;
        let output_c = std::ffi::CString::new(output_path.to_string_lossy().into_owned())
            .map_err(|e| AppError::ColorGradingError(format!("Invalid output path: {}", e)))?;
        let log_c = log_space
            .map(|s| std::ffi::CString::new(s).map_err(|e| AppError::ColorGradingError(format!("Invalid log space string: {}", e))))
            .transpose()?
            .unwrap_or_else(|| {
                // Empty string is infallible for CString::new (no interior null bytes possible)
                std::ffi::CString::new("").expect("empty string is valid CString")
            });
        let metering_c = std::ffi::CString::new(metering_mode)
            .map_err(|e| AppError::ColorGradingError(format!("Invalid metering mode string: {}", e)))?;
        let lensfun_c = lensfun_db_path
            .map(|s| std::ffi::CString::new(s).map_err(|e| AppError::ColorGradingError(format!("Invalid lensfun path string: {}", e))))
            .transpose()?;

        let result = unsafe {
            (self.process_file_with_lut)(
                input_c.as_ptr(),
                output_c.as_ptr(),
                if log_space.is_some() {
                    log_c.as_ptr()
                } else {
                    std::ptr::null()
                },
                lut_data.table.as_ptr(),
                lut_data.size as c_int,
                lut_data.domain_min.as_ptr(),
                lut_data.domain_max.as_ptr(),
                metering_c.as_ptr(),
                ev_offset,
                DEFAULT_JPEG_QUALITY,
                ENABLE_LENS_CORRECTION,
                lensfun_c
                    .as_ref()
                    .map(|c| c.as_ptr())
                    .unwrap_or(std::ptr::null()),
                if enable_nn_demosaic { 1 } else { 0 },
            )
        };

        let ra_result = ra_result_from_code(result);

        if ra_result.is_ok() {
            Ok(())
        } else {
            Err(self.format_last_error(ra_result, result))
        }
    }

    /// Eagerly initialize the NN demosaic session. Intended to be called from a
    /// background thread at app launch so the QNN graph compile overlaps with
    /// browsing. Best-effort; failures are logged in C++ and swallowed. No-op
    /// (debug log) if the build does not export `raWarmupNnSession`.
    pub fn warmup_nn_session(&self) {
        match self.warmup_nn_session {
            Some(f) => unsafe { f() },
            None => tracing::debug!("raWarmupNnSession unavailable — skipping background warmup"),
        }
    }

    /// True iff the NN session successfully initialized (NPU engaged). Used by
    /// the color-grading router to tell structural NN unavailability from a
    /// per-file NN error on a ready session.
    pub fn is_nn_ready(&self) -> bool {
        match self.is_nn_ready {
            Some(f) => unsafe { f() },
            None => false, // symbol absent → treat as "not ready" → classical
        }
    }

    /// Inject NN runtime config (model paths, QNN SoC params, DirectML path)
    /// into the C++ core via the explicit C-ABI transport. Must be called
    /// before warmup/decode so the config is in place when the NN session
    /// initializes. The C side deep-copies every field, so the caller's
    /// `RaNnConfig` (and any CString temporaries behind its pointers) may be
    /// dropped immediately after this returns. Returns an error if the build
    /// does not export `ra_set_nn_config`, or if the C side reports a failure
    /// (e.g. out-of-memory during the deep copy).
    pub fn set_nn_config(&self, cfg: &RaNnConfig) -> Result<(), AppError> {
        let f = self.set_nn_config.ok_or_else(|| {
            AppError::ColorGradingError("ra_set_nn_config unavailable in this build".into())
        })?;
        let result = unsafe { f(cfg) };
        if result == 0 {
            Ok(())
        } else {
            Err(AppError::ColorGradingError(format!(
                "ra_set_nn_config failed (code {})",
                result
            )))
        }
    }

    /// Supply an NN model's ONNX weights as an in-memory byte buffer (Option D).
    /// `kind`: 0 = bayer, 1 = xtrans. The C side deep-copies, so `data` may be
    /// dropped immediately after this returns. Pass an empty slice to mark the
    /// model absent. Returns an error if the build does not export
    /// `ra_set_nn_model`, or if the C side reports a failure.
    pub fn set_nn_model(&self, kind: i32, data: &[u8]) -> Result<(), AppError> {
        let f = self.set_nn_model.ok_or_else(|| {
            AppError::ColorGradingError("ra_set_nn_model unavailable in this build".into())
        })?;
        let result = unsafe {
            f(
                kind as c_int,
                data.as_ptr() as *const std::ffi::c_void,
                data.len(),
            )
        };
        if result == 0 {
            Ok(())
        } else {
            Err(AppError::ColorGradingError(format!(
                "ra_set_nn_model(kind={}) failed (code {})",
                kind, result
            )))
        }
    }

    /// Redirect C++ NN diagnostics (nnlog::info) into `path` (opened in append
    /// mode). Pass `None` to revert the C++ side to stderr. No-op (debug log)
    /// if the build does not export `ra_set_log_file`.
    pub fn set_log_file(&self, path: Option<&str>) {
        match self.set_log_file {
            Some(f) => {
                let cstr = path.map(std::ffi::CString::new);
                match cstr {
                    Some(Ok(c)) => unsafe { f(c.as_ptr()) },
                    // NUL in path: log and skip rather than panic; diagnostics
                    // just stay on whatever was previously configured.
                    Some(Err(e)) => tracing::warn!("set_log_file: invalid path ({}); skipped", e),
                    None => unsafe { f(std::ptr::null()) },
                }
            }
            None => tracing::debug!("ra_set_log_file unavailable — C++ NN diagnostics stay on stderr"),
        }
    }

    pub(crate) fn begin_preview_session(
        &self,
        input_path: &Path,
        lensfun_db_path: Option<&str>,
        half_size: bool,
        max_preview_width: u32,
        max_preview_height: u32,
    ) -> Result<RaPreviewSession, AppError> {
        let input_c = std::ffi::CString::new(input_path.to_string_lossy().into_owned())
            .map_err(|e| AppError::ColorGradingError(format!("Invalid input path: {}", e)))?;
        let lensfun_c = lensfun_db_path
            .map(|s| std::ffi::CString::new(s).map_err(|e| AppError::ColorGradingError(format!("Invalid lensfun path string: {}", e))))
            .transpose()?;

        let mut session = RaPreviewSession { ptr: std::ptr::null_mut() };

        let result = unsafe {
            (self.begin_preview_session)(
                input_c.as_ptr(),
                ENABLE_LENS_CORRECTION,
                lensfun_c
                    .as_ref()
                    .map(|c| c.as_ptr())
                    .unwrap_or(std::ptr::null()),
                if half_size { 1 } else { 0 },
                max_preview_width as c_int,
                max_preview_height as c_int,
                &mut session,
            )
        };

        let ra_result = ra_result_from_code(result);
        if ra_result.is_ok() {
            Ok(session)
        } else {
            Err(self.format_last_error(ra_result, result))
        }
    }

    pub(crate) fn apply_preview_grading(
        &self,
        session: &RaPreviewSession,
        log_space: Option<&str>,
        lut_data: &Arc<super::lut_data::LutData>,
        ev_offset: f32,
        metering_mode: &str,
        jpeg_quality: i32,
        max_width: u32,
        max_height: u32,
    ) -> Result<Vec<u8>, AppError> {
        let log_c = log_space
            .map(|s| std::ffi::CString::new(s).map_err(|e| AppError::ColorGradingError(format!("Invalid log space: {}", e))))
            .transpose()?
            .unwrap_or_else(|| std::ffi::CString::new("").expect("empty string is valid CString"));
        let metering_c = std::ffi::CString::new(metering_mode)
            .map_err(|e| AppError::ColorGradingError(format!("Invalid metering mode: {}", e)))?;

        let mut out_buf: *mut u8 = std::ptr::null_mut();
        let mut out_len: c_int = 0;

        let result = unsafe {
            (self.apply_preview_grading)(
                session.ptr,
                if log_space.is_some() { log_c.as_ptr() } else { std::ptr::null() },
                lut_data.table.as_ptr(),
                lut_data.size as c_int,
                lut_data.domain_min.as_ptr(),
                lut_data.domain_max.as_ptr(),
                metering_c.as_ptr(),
                ev_offset,
                jpeg_quality as c_int,
                max_width as c_int,
                max_height as c_int,
                &mut out_buf,
                &mut out_len,
            )
        };

        let ra_result = ra_result_from_code(result);
        if !ra_result.is_ok() {
            return Err(self.format_last_error(ra_result, result));
        }

        if out_buf.is_null() || out_len <= 0 {
            return Err(AppError::ColorGradingError("Buffer is empty".into()));
        }

        let _guard = CppBufferGuard { buf: out_buf, lib: self };
        let jpeg_bytes = unsafe {
            std::slice::from_raw_parts(out_buf, out_len as usize).to_vec()
        };
        // Guard drops here, freeing the C++ buffer even if to_vec() panics

        Ok(jpeg_bytes)
    }

    pub(crate) fn end_preview_session(&self, session: RaPreviewSession) {
        if !session.ptr.is_null() {
            unsafe {
                (self.end_preview_session)(session.ptr);
            }
        }
    }
}

/// Eagerly initialize the NN demosaic session from a background thread at app
/// launch so the ~2s QNN graph compile overlaps with browsing. Fire-and-forget
/// best-effort: any failure is logged and swallowed in C++; the edit path
/// re-attempts via decodeRawNn if this didn't succeed. Thread-safe by the
/// singleton's init() mutex, so a concurrent first edit just observes ready.
pub fn warmup_nn_session() {
    match RawAlchemyLib::get() {
        Ok(lib) => lib.warmup_nn_session(),
        Err(e) => tracing::warn!("NN warmup skipped (lib not loaded): {}", e),
    }
}

/// Whether the NN demosaic session is ready (NPU engaged). Returns false if the
/// native lib isn't loaded or the symbol is absent. Used by the service router
/// for fallback decisions (structural unavailability vs. per-file error).
pub fn is_nn_ready() -> bool {
    match RawAlchemyLib::get() {
        Ok(lib) => lib.is_nn_ready(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ra_result_is_ok_only_for_ok_variant() {
        assert!(RaResult::Ok.is_ok());

        assert!(!RaResult::ErrUnknown.is_ok());
        assert!(!RaResult::ErrFileNotFound.is_ok());
        assert!(!RaResult::ErrDecodeFailed.is_ok());
        assert!(!RaResult::ErrInvalidParam.is_ok());
        assert!(!RaResult::ErrLogUnsupported.is_ok());
        assert!(!RaResult::ErrLutLoadFailed.is_ok());
        assert!(!RaResult::ErrWriteFailed.is_ok());
        assert!(!RaResult::ErrNoLensProfile.is_ok());
        assert!(!RaResult::ErrOutOfMemory.is_ok());
        assert!(!RaResult::ErrNnNotInitialized.is_ok());
        assert!(!RaResult::ErrNnNanOutput.is_ok());
        assert!(!RaResult::ErrNnInferenceFailed.is_ok());
    }

    #[test]
    fn ra_result_description_returns_non_empty() {
        let variants = [
            RaResult::Ok,
            RaResult::ErrUnknown,
            RaResult::ErrFileNotFound,
            RaResult::ErrDecodeFailed,
            RaResult::ErrInvalidParam,
            RaResult::ErrLogUnsupported,
            RaResult::ErrLutLoadFailed,
            RaResult::ErrWriteFailed,
            RaResult::ErrNoLensProfile,
            RaResult::ErrOutOfMemory,
            RaResult::ErrNnNotInitialized,
            RaResult::ErrNnNanOutput,
            RaResult::ErrNnInferenceFailed,
        ];

        let descriptions: Vec<&str> = variants.iter().map(|v| v.description()).collect();

        for desc in &descriptions {
            assert!(!desc.is_empty(), "Description should not be empty");
        }

        // Verify all descriptions are distinct
        for i in 0..descriptions.len() {
            for j in (i + 1)..descriptions.len() {
                assert_ne!(
                    descriptions[i], descriptions[j],
                    "Descriptions for {:?} and {:?} should differ",
                    variants[i], variants[j]
                );
            }
        }
    }

    #[test]
    fn ra_result_repr_values() {
        assert_eq!(RaResult::Ok as i32, 0);
        assert_eq!(RaResult::ErrUnknown as i32, -1);
        assert_eq!(RaResult::ErrFileNotFound as i32, -2);
        assert_eq!(RaResult::ErrDecodeFailed as i32, -3);
        assert_eq!(RaResult::ErrInvalidParam as i32, -4);
        assert_eq!(RaResult::ErrLogUnsupported as i32, -5);
        assert_eq!(RaResult::ErrLutLoadFailed as i32, -6);
        assert_eq!(RaResult::ErrWriteFailed as i32, -7);
        assert_eq!(RaResult::ErrNoLensProfile as i32, -8);
        assert_eq!(RaResult::ErrOutOfMemory as i32, -9);
        assert_eq!(RaResult::ErrNnNotInitialized as i32, -10);
        assert_eq!(RaResult::ErrNnNanOutput as i32, -11);
        assert_eq!(RaResult::ErrNnInferenceFailed as i32, -12);
    }

    #[test]
    fn ra_result_from_code_maps_all_known_values() {
        assert_eq!(ra_result_from_code(0), RaResult::Ok);
        assert_eq!(ra_result_from_code(-1), RaResult::ErrUnknown);
        assert_eq!(ra_result_from_code(-2), RaResult::ErrFileNotFound);
        assert_eq!(ra_result_from_code(-3), RaResult::ErrDecodeFailed);
        assert_eq!(ra_result_from_code(-4), RaResult::ErrInvalidParam);
        assert_eq!(ra_result_from_code(-5), RaResult::ErrLogUnsupported);
        assert_eq!(ra_result_from_code(-6), RaResult::ErrLutLoadFailed);
        assert_eq!(ra_result_from_code(-7), RaResult::ErrWriteFailed);
        assert_eq!(ra_result_from_code(-8), RaResult::ErrNoLensProfile);
        assert_eq!(ra_result_from_code(-9), RaResult::ErrOutOfMemory);
        assert_eq!(ra_result_from_code(-10), RaResult::ErrNnNotInitialized);
        assert_eq!(ra_result_from_code(-11), RaResult::ErrNnNanOutput);
        assert_eq!(ra_result_from_code(-12), RaResult::ErrNnInferenceFailed);
    }

    #[test]
    fn ra_result_from_code_unknown_value_falls_back() {
        assert_eq!(ra_result_from_code(-99), RaResult::ErrUnknown);
        assert_eq!(ra_result_from_code(42), RaResult::ErrUnknown);
    }
}
