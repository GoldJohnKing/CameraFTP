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
    const ONNXRUNTIME_DLL_GZ: &[u8] =
        include_bytes!(concat!(env!("OUT_DIR"), "/onnxruntime.dll.gz"));
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
    pub fn preload_onnxruntime() -> Result<(), AppError> {
        preload_dll_by_name(ONNXRUNTIME_DLL_GZ, "onnxruntime.dll")?;
        Ok(())
    }

    /// Preload DirectML.dll and publish its path via the `RA_NN_DIRECTML_DLL`
    /// env var. ORT's DirectML EP loads DirectML.dll at runtime (DMLCreateDevice);
    /// the exact-name preload makes that bind to our copy, and the env var lets
    /// the C++ core (nn_session.cpp) call `SetDllDirectoryA` on the same dir as
    /// defense-in-depth against a stale System32 DirectML.dll (ORT issue #18831).
    pub fn preload_directml() -> Result<(), AppError> {
        let dll_path = preload_dll_by_name(DIRECTML_DLL_GZ, "DirectML.dll")?;
        // Read by the C++ core (raw_alchemy_capi.cpp) into DecodeParams
        // .nnDirectmlDllPath, then used by nn_session.cpp's SetDllDirectoryA.
        // Set unconditionally so the C++ side always points at our extraction dir.
        let path_str = dll_path.to_string_lossy().into_owned();
        std::env::set_var("RA_NN_DIRECTML_DLL", &path_str);
        tracing::debug!("Set RA_NN_DIRECTML_DLL={}", path_str);
        Ok(())
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

// Optional symbol: only present when the C++ core is built with NN demosaic
// enabled (RA_ENABLE_NN_DEMOSAIC). Resolved lazily/optionally so library
// loading stays robust on builds that don't export it — the NN init call site
// then degrades to a non-fatal warning with classical-demosaic fallback.
// No args: the C++ side reads RA_NN_BAYER_MODEL / RA_NN_XTRANS_MODEL env vars
// for model paths (set by the Rust startup before this runs).
type RaDemosaicNnInitFn = unsafe extern "C" fn() -> c_int;

pub struct RawAlchemyLib {
    _lib: Library,
    process_file_with_lut: RaProcessFileWithLUTFn,
    get_last_error: RaGetLastErrorFn,
    get_version: RaGetVersionFn,
    begin_preview_session: RaBeginPreviewSessionFn,
    apply_preview_grading: RaApplyPreviewGradingFn,
    end_preview_session: RaEndPreviewSessionFn,
    free_preview_buffer: RaFreePreviewBufferFn,
    demosaic_nn_init: Option<RaDemosaicNnInitFn>,
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

        // NN init is optional: builds without RA_ENABLE_NN_DEMOSAIC don't export
        // it. A failed lookup here is expected and non-fatal — demosaic_nn_init()
        // returns an error and callers fall back to classical demosaic.
        let demosaic_nn_init = unsafe {
            lib.get::<RaDemosaicNnInitFn>(b"raDemosaicNnInit\0")
                .ok()
                .map(|f| *f)
        };
        if demosaic_nn_init.is_some() {
            tracing::debug!("raDemosaicNnInit symbol resolved");
        } else {
            tracing::debug!("raDemosaicNnInit symbol not present — NN demosaic disabled in this build");
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
            demosaic_nn_init,
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

    /// Initialize the NN demosaic session once at startup.
    ///
    /// Non-fatal by design: callers wrap the `Result` in a warning and fall
    /// back to classical demosaic. Model paths are supplied to the C++ side via
    /// the `RA_NN_BAYER_MODEL` / `RA_NN_XTRANS_MODEL` env vars (see
    /// `resources::configure_nn_model_env`), which must be set before this call.
    /// Returns an error if the build does not export `raDemosaicNnInit`.
    pub fn demosaic_nn_init(&self) -> Result<(), AppError> {
        let init = self.demosaic_nn_init.ok_or_else(|| {
            AppError::ColorGradingError(
                "NN demosaic init unavailable in this build — classical path will be used".into(),
            )
        })?;
        let result = unsafe { (init)() };
        let ra_result = ra_result_from_code(result);
        if ra_result.is_ok() {
            tracing::info!("NN demosaic session initialized");
            Ok(())
        } else {
            Err(self.format_last_error(ra_result, result))
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
