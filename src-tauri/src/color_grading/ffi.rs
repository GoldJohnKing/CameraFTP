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
            cleanup_old_dlls(&temp_dir, &dll_name);
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

        cleanup_old_dlls(&temp_dir, &dll_name);

        Ok(dll_path)
    }

    /// Remove old versions of the extracted DLL from the temp directory.
    fn cleanup_old_dlls(temp_dir: &Path, current_name: &str) {
        if let Ok(entries) = std::fs::read_dir(temp_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();
                if name_str.starts_with("raw_alchemy_core_")
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

type RaCommitPreviewFn = unsafe extern "C" fn(
    *mut std::ffi::c_void, // session
    *const c_char,         // logSpace
    *const c_float,        // lutTable
    c_int,                 // lutSize
    *const c_float,        // lutDomainMin
    *const c_float,        // lutDomainMax
    *const c_char,         // metering
    c_float,               // evOffset
    c_int,                 // jpegQuality
    *const c_char,         // outputPath
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
    commit_preview: RaCommitPreviewFn,
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

        let commit_preview = unsafe {
            *lib.get::<RaCommitPreviewFn>(b"raCommitPreview\0")
                .map_err(|e| {
                    AppError::ColorGradingError(format!("Symbol raCommitPreview not found: {}", e))
                })?
        };

        Ok(Self {
            _lib: lib,
            process_file_with_lut,
            get_last_error,
            get_version,
            begin_preview_session,
            apply_preview_grading,
            end_preview_session,
            free_preview_buffer,
            commit_preview,
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
            )
        };

        let ra_result = ra_result_from_code(result);

        if ra_result.is_ok() {
            Ok(())
        } else {
            Err(self.format_last_error(ra_result, result))
        }
    }
    pub(crate) fn begin_preview_session(
        &self,
        input_path: &Path,
        enable_lens_correction: bool,
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
                if enable_lens_correction { 1 } else { 0 },
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

    pub(crate) fn commit_preview(
        &self,
        session: &RaPreviewSession,
        log_space: Option<&str>,
        lut_data: &Arc<super::lut_data::LutData>,
        ev_offset: f32,
        metering_mode: &str,
        jpeg_quality: i32,
        output_path: &Path,
    ) -> Result<(), AppError> {
        let log_c = log_space
            .map(|s| std::ffi::CString::new(s).map_err(|e| AppError::ColorGradingError(format!("Invalid log space: {}", e))))
            .transpose()?
            .unwrap_or_else(|| std::ffi::CString::new("").expect("empty string is valid CString"));
        let metering_c = std::ffi::CString::new(metering_mode)
            .map_err(|e| AppError::ColorGradingError(format!("Invalid metering mode: {}", e)))?;
        let output_c = std::ffi::CString::new(output_path.to_string_lossy().into_owned())
            .map_err(|e| AppError::ColorGradingError(format!("Invalid output path: {}", e)))?;

        let result = unsafe {
            (self.commit_preview)(
                session.ptr,
                if log_space.is_some() { log_c.as_ptr() } else { std::ptr::null() },
                lut_data.table.as_ptr(),
                lut_data.size as c_int,
                lut_data.domain_min.as_ptr(),
                lut_data.domain_max.as_ptr(),
                metering_c.as_ptr(),
                ev_offset,
                jpeg_quality as c_int,
                output_c.as_ptr(),
            )
        };

        let ra_result = ra_result_from_code(result);
        if ra_result.is_ok() {
            Ok(())
        } else {
            Err(self.format_last_error(ra_result, result))
        }
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
    }

    #[test]
    fn ra_result_from_code_unknown_value_falls_back() {
        assert_eq!(ra_result_from_code(-99), RaResult::ErrUnknown);
        assert_eq!(ra_result_from_code(42), RaResult::ErrUnknown);
    }
}
