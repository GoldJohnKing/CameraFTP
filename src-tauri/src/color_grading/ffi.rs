// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::os::raw::{c_char, c_float, c_int};
use std::path::Path;
use std::sync::{Arc, OnceLock};

use libloading::Library;

use crate::error::AppError;

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

type RaProcessFileFn = unsafe extern "C" fn(
    *const c_char,
    *const c_char,
    *const c_char,
    *const c_char,
    *const c_char,
    c_float,
    c_int,
    c_int,
    c_int,
    *const c_char,
) -> c_int;

type RaProcessFileWithLUTFn = unsafe extern "C" fn(
    *const c_char,   // inputPath
    *const c_char,   // outputPath
    *const c_char,   // logSpace
    *const c_float,  // lutTable
    c_int,           // lutSize
    *const c_float,  // lutDomainMin
    *const c_float,  // lutDomainMax
    *const c_char,   // metering
    c_float,         // manualEv
    c_int,           // useAutoExposure
    c_int,           // jpegQuality
    c_int,           // enableLensCorrection
    *const c_char,   // customLensfunDb
) -> c_int;

type RaGetLastErrorFn = unsafe extern "C" fn() -> *const c_char;
type RaGetVersionFn = unsafe extern "C" fn() -> *const c_char;

pub struct RawAlchemyLib {
    _lib: Library,
    process_file: RaProcessFileFn,
    process_file_with_lut: RaProcessFileWithLUTFn,
    get_last_error: RaGetLastErrorFn,
    get_version: RaGetVersionFn,
}

static GLOBAL_LIB: OnceLock<Arc<RawAlchemyLib>> = OnceLock::new();

impl RawAlchemyLib {
    pub fn load(path: &Path) -> Result<Self, AppError> {
        let lib = unsafe {
            Library::new(path).map_err(|e| {
                AppError::ColorGradingError(format!("Failed to load {}: {}", path.display(), e))
            })?
        };

        let process_file = unsafe {
            *lib.get::<RaProcessFileFn>(b"raProcessFile\0")
                .map_err(|e| {
                    AppError::ColorGradingError(format!("Symbol raProcessFile not found: {}", e))
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

        Ok(Self {
            _lib: lib,
            process_file,
            process_file_with_lut,
            get_last_error,
            get_version,
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

    pub fn process_file(
        &self,
        input_path: &Path,
        output_path: &Path,
        log_space: Option<&str>,
        lut_path: Option<&str>,
        lensfun_db_path: Option<&str>,
    ) -> Result<(), AppError> {
        let input_c = std::ffi::CString::new(input_path.to_string_lossy().into_owned())
            .map_err(|e| AppError::ColorGradingError(format!("Invalid input path: {}", e)))?;
        let output_c = std::ffi::CString::new(output_path.to_string_lossy().into_owned())
            .map_err(|e| AppError::ColorGradingError(format!("Invalid output path: {}", e)))?;
        let log_c = log_space
            .map(|s| std::ffi::CString::new(s).unwrap())
            .unwrap_or_else(|| std::ffi::CString::new("").unwrap());
        let lut_c = lut_path
            .map(|s| std::ffi::CString::new(s).unwrap())
            .unwrap_or_else(|| std::ffi::CString::new("").unwrap());
        let metering_c = std::ffi::CString::new("matrix").unwrap();
        let lensfun_c = lensfun_db_path.map(|s| std::ffi::CString::new(s).unwrap());

        let result = unsafe {
            (self.process_file)(
                input_c.as_ptr(),
                output_c.as_ptr(),
                if log_space.is_some() {
                    log_c.as_ptr()
                } else {
                    std::ptr::null()
                },
                if lut_path.is_some() {
                    lut_c.as_ptr()
                } else {
                    std::ptr::null()
                },
                metering_c.as_ptr(),
                0.0, // manualEv — ignored when useAutoExposure=1
                1,   // useAutoExposure
                95,  // jpegQuality
                1,   // enableLensCorrection
                lensfun_c
                    .as_ref()
                    .map(|c| c.as_ptr())
                    .unwrap_or(std::ptr::null()),
            )
        };

        let ra_result = unsafe { std::mem::transmute::<c_int, RaResult>(result) };

        if ra_result.is_ok() {
            Ok(())
        } else {
            let last_error = unsafe {
                let ptr = (self.get_last_error)();
                if ptr.is_null() {
                    String::new()
                } else {
                    std::ffi::CStr::from_ptr(ptr)
                        .to_string_lossy()
                        .into_owned()
                }
            };
            Err(AppError::ColorGradingError(if last_error.is_empty() {
                format!("{} ({})", ra_result.description(), result)
            } else {
                format!("{}: {}", ra_result.description(), last_error)
            }))
        }
    }

    pub fn process_file_with_lut(
        &self,
        input_path: &Path,
        output_path: &Path,
        log_space: Option<&str>,
        lut_data: &super::lut_data::LutData,
        lensfun_db_path: Option<&str>,
    ) -> Result<(), AppError> {
        let input_c = std::ffi::CString::new(input_path.to_string_lossy().into_owned())
            .map_err(|e| AppError::ColorGradingError(format!("Invalid input path: {}", e)))?;
        let output_c = std::ffi::CString::new(output_path.to_string_lossy().into_owned())
            .map_err(|e| AppError::ColorGradingError(format!("Invalid output path: {}", e)))?;
        let log_c = log_space
            .map(|s| std::ffi::CString::new(s).unwrap())
            .unwrap_or_else(|| std::ffi::CString::new("").unwrap());
        let metering_c = std::ffi::CString::new("highlight-safe").unwrap();
        let lensfun_c = lensfun_db_path.map(|s| std::ffi::CString::new(s).unwrap());

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
                0.0,
                1,
                95,
                1,
                lensfun_c
                    .as_ref()
                    .map(|c| c.as_ptr())
                    .unwrap_or(std::ptr::null()),
            )
        };

        let ra_result = unsafe { std::mem::transmute::<c_int, RaResult>(result) };

        if ra_result.is_ok() {
            Ok(())
        } else {
            let last_error = unsafe {
                let ptr = (self.get_last_error)();
                if ptr.is_null() {
                    String::new()
                } else {
                    std::ffi::CStr::from_ptr(ptr)
                        .to_string_lossy()
                        .into_owned()
                }
            };
            Err(AppError::ColorGradingError(if last_error.is_empty() {
                format!("{} ({})", ra_result.description(), result)
            } else {
                format!("{}: {}", ra_result.description(), last_error)
            }))
        }
    }
}
