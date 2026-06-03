# Real-time Color Grading Preview — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a two-phase C++ session API and Rust FFI layer for real-time color grading preview, caching decoded RAW data to enable ~150-200ms parameter changes.

**Architecture:** C++ provides session-based API (`raBeginPreviewSession` / `raApplyPreviewGrading` / `raEndPreviewSession`). Rust orchestrates session lifecycle via Tauri commands. Preview JPEG is written to a temp file and served through the existing `image-preview://` URI scheme.

**Tech Stack:** C++17 (CMake), Rust (Tauri v2), FFI via `libloading`

**Design Spec:** `docs/superpowers/specs/2026-06-03-realtime-color-grading-preview-design.md`

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `src-tauri/lib/rawalchemy/include/raw_alchemy_capi.h` | Modify | Session type + 3 API declarations |
| `src-tauri/lib/rawalchemy/src/raw_alchemy_capi.cpp` | Modify | Session struct + 3 API implementations + grading-only helper |
| `src-tauri/src/color_grading/ffi.rs` | Modify | 3 new FFI symbol types + loading + wrapper methods |
| `src-tauri/src/color_grading/preview.rs` | Create | Preview session state management |
| `src-tauri/src/color_grading/mod.rs` | Modify | Add `pub mod preview` |
| `src-tauri/src/commands/color_grading.rs` | Modify | 3 new Tauri commands |
| `src-tauri/src/lib.rs` | Modify | Register commands + manage preview state |

---

### Task 1: C++ Header — Add Preview Session API

**Files:**
- Modify: `src-tauri/lib/rawalchemy/include/raw_alchemy_capi.h`

- [ ] **Step 1: Add preview session declarations after `raProcessToBuffer` and before the Utility section**

Insert after the closing `}` of the `raProcessToBuffer` doc comment block (after line 167), before the `/* ---------------------------------------------------------------- *  Utility` section (line 169):

```c
/* ----------------------------------------------------------------
 *  Preview Session — two-phase preview pipeline
 *
 *  Decodes RAW + applies lens correction once, then allows fast
 *  re-grading with different LUT/exposure parameters.
 * ---------------------------------------------------------------- */
typedef struct RaPreviewSession_* RaPreviewSession;

/** Decode a RAW file and apply lens correction, caching the result for
 *  fast re-grading.  Call raEndPreviewSession when done.
 *
 *  @param inputPath           UTF-8 path to input RAW file.
 *  @param enableLensCorrection  If non-zero, apply lens correction.
 *  @param customLensfunDb      Custom Lensfun DB path, or NULL.
 *  @param outSession          Receives the session handle.
 *  @return RA_OK on success. */
RA_API RaResult RA_CALL raBeginPreviewSession(
    const char* inputPath,
    int         enableLensCorrection,
    const char* customLensfunDb,
    RaPreviewSession* outSession
);

/** Apply grading parameters to the session's cached decoded image.
 *
 *  The session's internal data is NOT modified — safe to call repeatedly
 *  with different parameters.  Internally clones the cached buffer, applies
 *  the full grading pipeline, and writes the result to outputPath.
 *
 *  Pipeline on cloned data:
 *    Exposure -> Sat/Contrast Boost -> Log Transform -> LUT -> JPEG encode
 *
 *  @param session         Active preview session.
 *  @param logSpace        Log space name, or NULL to skip.
 *  @param lutTable        Pre-parsed LUT float data [size^3 x 3], or NULL.
 *  @param lutSize         LUT dimension. Ignored if lutTable is NULL.
 *  @param lutDomainMin    LUT domain min [R,G,B], or NULL for {0,0,0}.
 *  @param lutDomainMax    LUT domain max [R,G,B], or NULL for {1,1,1}.
 *  @param metering        Metering mode, or NULL for "matrix".
 *  @param manualEv        Manual exposure in stops.
 *  @param useAutoExposure If non-zero, use auto metering.
 *  @param jpegQuality     JPEG quality 1-100.
 *  @param outputPath      UTF-8 output path.
 *  @return RA_OK on success. */
RA_API RaResult RA_CALL raApplyPreviewGrading(
    RaPreviewSession session,
    const char*      logSpace,
    const float*     lutTable,
    int              lutSize,
    const float*     lutDomainMin,
    const float*     lutDomainMax,
    const char*      metering,
    float            manualEv,
    int              useAutoExposure,
    int              jpegQuality,
    const char*      outputPath
);

/** End a preview session and release all cached resources.
 *  Safe to pass NULL (no-op). */
RA_API void RA_CALL raEndPreviewSession(RaPreviewSession session);
```

- [ ] **Step 2: Commit**

```bash
git add src-tauri/lib/rawalchemy/include/raw_alchemy_capi.h
git commit -m "feat(color-grading): add preview session API declarations"
```

---

### Task 2: C++ Implementation — Session Pipeline

**Files:**
- Modify: `src-tauri/lib/rawalchemy/src/raw_alchemy_capi.cpp`

- [ ] **Step 1: Add session struct definition and grading-only helper**

Insert the session struct and `runGradingOnly` helper after the existing `runPipelineWithLUT` function (after line 307) and before the closing `} // anonymous namespace` (line 309):

```cpp
// ----------------------------------------------------------------
//  Preview Session
// ----------------------------------------------------------------
struct RaPreviewSession_ {
    rawalchemy::ImageBuffer decodedImage;   // post-lens-correction, pre-grading
    std::string inputPath;
};

/// Apply grading pipeline (exposure → sat/contrast → log → LUT) to an image.
/// Does NOT decode or apply lens correction — used for preview re-grading.
/// If logSpace is set and ARM64, uses float16 path for log+LUT.
RaResult runGradingOnly(
    rawalchemy::ImageBuffer& img,
    const char* logSpace,
    const rawalchemy::LUT3D* lut,
    const char* metering,
    float manualEv,
    int useAutoExposure)
{
    #if defined(__aarch64__)
    rawalchemy::HalfImageBuffer imgF16;
    bool usingF16 = false;
    #endif

    // Exposure
    try {
        if (useAutoExposure) {
            std::string mode(metering ? metering : "matrix");
            if (!rawalchemy::isMeteringModeSupported(mode)) {
                setError(std::string("Unsupported metering mode: ") + mode);
                return RA_ERR_INVALID_PARAM;
            }
            float gain = rawalchemy::computeAutoGain(img, mode);
            img.applyGain(gain);
        } else {
            img.applyGain(std::pow(2.0f, manualEv));
        }
    } catch (...) {
        return catchExceptions("exposure");
    }

    // Saturation / Contrast boost
    try {
        rawalchemy::applySaturationContrast(img, 1.25f, 1.10f);
    } catch (...) {
        return catchExceptions("saturation/contrast");
    }

    // Log transform
    if (logSpace) {
        try {
            std::string space(logSpace);
            if (!rawalchemy::isLogSpaceSupported(space)) {
                setError(std::string("Unsupported log space: ") + logSpace);
                return RA_ERR_LOG_UNSUPPORTED;
            }
            rawalchemy::applyGamutTransform(img, space);

            #if defined(__aarch64__)
            imgF16 = rawalchemy::convertToF16(img);
            rawalchemy::applyLogEncodingF16(imgF16, space);
            usingF16 = true;
            #else
            rawalchemy::applyLogEncoding(img, space);
            #endif
        } catch (...) {
            return catchExceptions("log transform");
        }
    }

    // LUT
    if (lut && !lut->empty()) {
        try {
            #if defined(__aarch64__)
            if (usingF16) {
                rawalchemy::applyLUT3DF16(imgF16, *lut);
            } else {
                rawalchemy::applyLUT3D(img, *lut);
            }
            #else
            rawalchemy::applyLUT3D(img, *lut);
            #endif
        } catch (...) {
            return catchExceptions("LUT");
        }
    }

    // Convert back from float16 before output
    #if defined(__aarch64__)
    if (usingF16) {
        img = rawalchemy::convertToF32(imgF16);
    }
    #endif

    return RA_OK;
}
```

- [ ] **Step 2: Add the 3 session API implementations**

Insert after the `raGetVersion` function (after line 516, at end of file):

```cpp

// ----------------------------------------------------------------
//  Preview Session
// ----------------------------------------------------------------

RA_API RaResult RA_CALL raBeginPreviewSession(
    const char* inputPath,
    int         enableLensCorrection,
    const char* customLensfunDb,
    RaPreviewSession* outSession)
{
    if (!inputPath || !outSession) {
        setError("raBeginPreviewSession: null parameter");
        return RA_ERR_INVALID_PARAM;
    }
    clearError();

    try {
        auto img = rawalchemy::decodeRaw(std::string(inputPath));
        auto meta = rawalchemy::extractMetadata(std::string(inputPath));

        if (enableLensCorrection) {
            try {
                rawalchemy::LensCorrectionParams lcParams;
                lcParams.enabled = true;
                lcParams.correctDistortion = true;
                lcParams.correctTca = true;
                lcParams.correctVignetting = true;
                lcParams.distance = 1000.0f;
                if (customLensfunDb) lcParams.customDbPath = customLensfunDb;
                rawalchemy::applyLensCorrection(img, meta, lcParams);
            } catch (...) {
                return catchExceptions("lens correction");
            }
        }

        *outSession = new RaPreviewSession_{
            std::move(img),
            std::string(inputPath)
        };
        return RA_OK;
    } catch (...) {
        return catchExceptions("raBeginPreviewSession");
    }
}

RA_API RaResult RA_CALL raApplyPreviewGrading(
    RaPreviewSession session,
    const char*      logSpace,
    const float*     lutTable,
    int              lutSize,
    const float*     lutDomainMin,
    const float*     lutDomainMax,
    const char*      metering,
    float            manualEv,
    int              useAutoExposure,
    int              jpegQuality,
    const char*      outputPath)
{
    if (!session || !outputPath) {
        setError("raApplyPreviewGrading: null parameter");
        return RA_ERR_INVALID_PARAM;
    }
    clearError();

    try {
        // Clone the cached decoded image so the original is preserved
        auto img = session->decodedImage;

        // Build LUT from pre-parsed data
        rawalchemy::LUT3D lut;
        const rawalchemy::LUT3D* lutPtr = nullptr;
        if (lutTable && lutSize > 0) {
            lut.size = lutSize;
            int totalFloats = lutSize * lutSize * lutSize * 3;
            lut.table.assign(lutTable, lutTable + totalFloats);
            if (lutDomainMin) {
                lut.domainMin[0] = lutDomainMin[0];
                lut.domainMin[1] = lutDomainMin[1];
                lut.domainMin[2] = lutDomainMin[2];
            }
            if (lutDomainMax) {
                lut.domainMax[0] = lutDomainMax[0];
                lut.domainMax[1] = lutDomainMax[1];
                lut.domainMax[2] = lutDomainMax[2];
            }
            lutPtr = &lut;
        }

        RaResult res = runGradingOnly(img, logSpace, lutPtr, metering,
                                       manualEv, useAutoExposure);
        if (res != RA_OK) return res;

        bool ok = rawalchemy::writeJpeg(img, std::string(outputPath), jpegQuality, false, nullptr);
        if (!ok) {
            setError("Failed to write preview JPEG");
            return RA_ERR_WRITE_FAILED;
        }
        return RA_OK;
    } catch (...) {
        return catchExceptions("raApplyPreviewGrading");
    }
}

RA_API void RA_CALL raEndPreviewSession(RaPreviewSession session) {
    delete session;
}
```

- [ ] **Step 3: Commit**

```bash
git add src-tauri/lib/rawalchemy/src/raw_alchemy_capi.cpp
git commit -m "feat(color-grading): implement preview session pipeline in C++"
```

---

### Task 3: Rebuild C++ DLL

- [ ] **Step 1: Build the C++ DLL with new symbols**

```bash
cd /mnt/d/GitRepos/CameraFTP && ./scripts/build-raw-alchemy.sh windows Release
```

Expected: `RawAlchemyCpp DLL built: .../build-windows-dll/bin/Release/raw_alchemy_core.dll`

- [ ] **Step 2: Verify new symbols exist in the DLL**

```bash
python3 -c "
import subprocess
result = subprocess.run(['dumpbin', '/exports', 'src-tauri/lib/rawalchemy/build-windows-dll/bin/Release/raw_alchemy_core.dll'], capture_output=True, text=True)
for sym in ['raBeginPreviewSession', 'raApplyPreviewGrading', 'raEndPreviewSession']:
    if sym in result.stdout:
        print(f'  FOUND: {sym}')
    else:
        print(f'  MISSING: {sym}')
" 2>/dev/null || echo "dumpbin not available, skipping symbol check (build success is sufficient)"
```

Expected: All 3 symbols found, or build succeeded without errors.

---

### Task 4: Rust FFI Layer — Session Symbols & Wrappers

**Files:**
- Modify: `src-tauri/src/color_grading/ffi.rs`

- [ ] **Step 1: Add session handle type and new FFI function signatures**

Insert after the `RaGetVersionFn` type alias (after line 147) and before the `pub struct RawAlchemyLib` definition (line 149):

```rust
/// Opaque handle to a C++ preview session (decoded RAW cached in C++ heap).
#[repr(transparent)]
pub(crate) struct RaPreviewSession {
    pub(crate) ptr: *mut std::ffi::c_void,
}

// SAFETY: The C++ session is accessed from Rust only via spawn_blocking,
// so only one thread uses a session at a time.
unsafe impl Send for RaPreviewSession {}

type RaBeginPreviewSessionFn = unsafe extern "C" fn(
    *const c_char,   // inputPath
    c_int,           // enableLensCorrection
    *const c_char,   // customLensfunDb
    *mut RaPreviewSession, // outSession
) -> c_int;

type RaApplyPreviewGradingFn = unsafe extern "C" fn(
    *mut std::ffi::c_void, // session (RaPreviewSession.ptr)
    *const c_char,   // logSpace
    *const c_float,  // lutTable
    c_int,           // lutSize
    *const c_float,  // lutDomainMin
    *const c_float,  // lutDomainMax
    *const c_char,   // metering
    c_float,         // manualEv
    c_int,           // useAutoExposure
    c_int,           // jpegQuality
    *const c_char,   // outputPath
) -> c_int;

type RaEndPreviewSessionFn = unsafe extern "C" fn(
    *mut std::ffi::c_void, // session (RaPreviewSession.ptr)
);
```

- [ ] **Step 2: Add new fields to `RawAlchemyLib` struct**

Replace the `RawAlchemyLib` struct definition (lines 149-154) with:

```rust
pub struct RawAlchemyLib {
    _lib: Library,
    process_file_with_lut: RaProcessFileWithLUTFn,
    get_last_error: RaGetLastErrorFn,
    get_version: RaGetVersionFn,
    begin_preview_session: RaBeginPreviewSessionFn,
    apply_preview_grading: RaApplyPreviewGradingFn,
    end_preview_session: RaEndPreviewSessionFn,
}
```

- [ ] **Step 3: Add symbol loading in `RawAlchemyLib::load`**

In the `load` function, after loading `get_version` (after line 202), add loading of the 3 new symbols:

```rust
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
```

Update the `Ok(Self { ... })` return to include the new fields:

```rust
        Ok(Self {
            _lib: lib,
            process_file_with_lut,
            get_last_error,
            get_version,
            begin_preview_session,
            apply_preview_grading,
            end_preview_session,
        })
```

- [ ] **Step 4: Add 3 wrapper methods to `RawAlchemyLib` impl**

Insert after the existing `process_file_with_lut` method (after line 317, before the `#[cfg(test)]` at line 320):

```rust
    pub fn begin_preview_session(
        &self,
        input_path: &Path,
        enable_lens_correction: bool,
        lensfun_db_path: Option<&str>,
    ) -> Result<RaPreviewSession, AppError> {
        let input_c = std::ffi::CString::new(input_path.to_string_lossy().into_owned())
            .map_err(|e| AppError::ColorGradingError(format!("Invalid input path: {}", e)))?;
        let lensfun_c = lensfun_db_path
            .map(|s| std::ffi::CString::new(s).map_err(|e| AppError::ColorGradingError(format!("Invalid lensfun path: {}", e))))
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

    pub fn apply_preview_grading(
        &self,
        session: &RaPreviewSession,
        log_space: Option<&str>,
        lut_data: &Arc<super::lut_data::LutData>,
        use_auto_exposure: bool,
        metering_mode: &str,
        manual_ev: f32,
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
            (self.apply_preview_grading)(
                session.ptr,
                if log_space.is_some() { log_c.as_ptr() } else { std::ptr::null() },
                lut_data.table.as_ptr(),
                lut_data.size as c_int,
                lut_data.domain_min.as_ptr(),
                lut_data.domain_max.as_ptr(),
                metering_c.as_ptr(),
                manual_ev,
                if use_auto_exposure { 1 } else { 0 },
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

    pub fn end_preview_session(&self, session: RaPreviewSession) {
        if !session.ptr.is_null() {
            unsafe {
                (self.end_preview_session)(session.ptr);
            }
        }
    }
```

- [ ] **Step 5: Verify Rust compilation**

```bash
cd /mnt/d/GitRepos/CameraFTP/src-tauri && cargo.exe check --target x86_64-pc-windows-msvc
```

Expected: Compiles without errors.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/color_grading/ffi.rs
git commit -m "feat(color-grading): add preview session FFI bindings"
```

---

### Task 5: Rust Preview State Module

**Files:**
- Create: `src-tauri/src/color_grading/preview.rs`
- Modify: `src-tauri/src/color_grading/mod.rs`

- [ ] **Step 1: Create `preview.rs` with session state management**

Create file `src-tauri/src/color_grading/preview.rs`:

```rust
// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::error::AppError;
use super::ffi::{RaPreviewSession, RawAlchemyLib};
use super::lut_data;
use super::presets::find_preset;

const PREVIEW_JPEG_QUALITY: i32 = 80;

struct ActiveSession {
    session: RaPreviewSession,
    image_path: String,
    preview_output_path: PathBuf,
}

pub struct ColorGradingPreviewState {
    inner: Mutex<Option<ActiveSession>>,
}

impl ColorGradingPreviewState {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(None),
        }
    }

    pub async fn begin(
        &self,
        image_path: &str,
        lensfun_db_path: Option<&str>,
    ) -> Result<(), AppError> {
        let lib = RawAlchemyLib::get()?;
        let input_path = Path::new(image_path);

        let mut guard = self.inner.lock().await;

        // End any existing session before starting a new one
        if let Some(active) = guard.take() {
            tracing::info!(old_image = %active.image_path, "Ending previous preview session");
            end_session_internal(&lib, active);
        }

        tracing::info!(image = image_path, "Beginning preview session (decoding RAW)...");

        let session = tokio::task::spawn_blocking({
            let input_path = input_path.to_path_buf();
            let lensfun = lensfun_db_path.map(String::from);
            move || {
                lib.begin_preview_session(
                    &input_path,
                    true,
                    lensfun.as_deref(),
                )
            }
        })
        .await
        .map_err(|e| AppError::ColorGradingError(format!("Blocking task failed: {}", e)))??;

        let preview_dir = std::env::temp_dir().join("CameraFTP").join("preview");
        tokio::fs::create_dir_all(&preview_dir).await
            .map_err(|e| AppError::ColorGradingError(format!("Failed to create preview dir: {}", e)))?;

        let preview_output_path = preview_dir.join(format!("preview_{}.jpg", session.ptr as usize));

        tracing::info!(image = image_path, "Preview session ready");

        *guard = Some(ActiveSession {
            session,
            image_path: image_path.to_string(),
            preview_output_path,
        });

        Ok(())
    }

    pub async fn apply(
        &self,
        lut_id: &str,
        use_auto_exposure: bool,
        metering_mode: &str,
        manual_ev: f32,
    ) -> Result<String, AppError> {
        let lib = RawAlchemyLib::get()?;
        let preset = find_preset(lut_id)
            .ok_or_else(|| AppError::ColorGradingError(format!("Unknown LUT preset: {}", lut_id)))?;
        let lut_data = lut_data::get_lut_data(&preset.id)?;

        let output_path = {
            let guard = self.inner.lock().await;
            let active = guard.as_ref()
                .ok_or_else(|| AppError::ColorGradingError("No active preview session".into()))?;
            active.preview_output_path.clone()
        };

        let log_space = preset.log_space.clone();
        let metering = metering_mode.to_string();

        tracing::debug!(lut = lut_id, ev = manual_ev, "Applying preview grading");

        tokio::task::spawn_blocking(move || {
            let session_ptr = {
                // Re-lock to get the session pointer (just copying the pointer value)
                // This is safe because we hold no lock during spawn_blocking
                // and the session is not modified by apply_preview_grading
                lib.apply_preview_grading(
                    // SAFETY: We need a reference to the session, but we can't
                    // hold the guard across spawn_blocking. The session pointer
                    // is stable as long as end_session is not called concurrently.
                    // Since we hold no lock here, but the caller guarantees
                    // no concurrent end/begin while apply is running.
                    unsafe { &*(&output_path as *const _) }, // placeholder, see below
                    // Actually we need the RaPreviewSession reference.
                    // Let's restructure...
                    &RaPreviewSession { ptr: std::ptr::null_mut() }, // will fix
                    log_space.as_deref(),
                    &lut_data,
                    use_auto_exposure,
                    &metering,
                    manual_ev,
                    PREVIEW_JPEG_QUALITY,
                    &output_path,
                )
            };
            output_path
        })
        .await
        .map_err(|e| AppError::ColorGradingError(format!("Blocking task failed: {}", e)))??;

        // Return URL for frontend
        let url = format!(
            "http://image-preview.localhost/{}",
            percent_encode(&output_path.to_string_lossy())
        );
        Ok(url)
    }

    pub async fn end(&self) -> Result<(), AppError> {
        let lib = RawAlchemyLib::get()?;
        let mut guard = self.inner.lock().await;

        if let Some(active) = guard.take() {
            tracing::info!(image = %active.image_path, "Ending preview session");
            end_session_internal(&lib, active);
        }

        Ok(())
    }
}

fn end_session_internal(lib: &Arc<RawAlchemyLib>, active: ActiveSession) {
    lib.end_preview_session(active.session);
    // Clean up temp file (best-effort)
    let _ = std::fs::remove_file(&active.preview_output_path);
}

/// Percent-encode a string for use in URI paths.
fn percent_encode(input: &str) -> String {
    let mut result = Vec::with_capacity(input.len());
    for &byte in input.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' | b':' | b'\\' => {
                result.push(byte);
            }
            _ => {
                result.extend_from_slice(format!("%{:02X}", byte).as_bytes());
            }
        }
    }
    String::from_utf8(result).unwrap_or_default()
}

impl Drop for ColorGradingPreviewState {
    fn drop(&mut self) {
        // Best-effort cleanup: try to end session synchronously on drop.
        // In practice, the Tauri app manages state lifecycle explicitly.
        if let Some(active) = self.inner.try_lock().ok().and_then(|mut g| g.take()) {
            if let Ok(lib) = RawAlchemyLib::get() {
                lib.end_preview_session(active.session);
                let _ = std::fs::remove_file(&active.preview_output_path);
            }
        }
    }
}
```

**Wait — there's a design flaw in `apply`.** The `spawn_blocking` closure can't hold a reference to `RaPreviewSession` from the MutexGuard. Let me fix this by passing the raw pointer:

Replace the `apply` method entirely with this corrected version:

```rust
    pub async fn apply(
        &self,
        lut_id: &str,
        use_auto_exposure: bool,
        metering_mode: &str,
        manual_ev: f32,
    ) -> Result<String, AppError> {
        let lib = RawAlchemyLib::get()?;
        let preset = find_preset(lut_id)
            .ok_or_else(|| AppError::ColorGradingError(format!("Unknown LUT preset: {}", lut_id)))?;
        let lut_data = lut_data::get_lut_data(&preset.id)?;

        // Copy what we need from the guard, then release the lock
        let (session_ptr, output_path) = {
            let guard = self.inner.lock().await;
            let active = guard.as_ref()
                .ok_or_else(|| AppError::ColorGradingError("No active preview session".into()))?;
            (active.session.ptr, active.preview_output_path.clone())
        };

        let log_space = preset.log_space.clone();
        let metering = metering_mode.to_string();

        tracing::debug!(lut = lut_id, ev = manual_ev, "Applying preview grading");

        tokio::task::spawn_blocking(move || {
            let session = RaPreviewSession { ptr: session_ptr };
            lib.apply_preview_grading(
                &session,
                log_space.as_deref(),
                &lut_data,
                use_auto_exposure,
                &metering,
                manual_ev,
                PREVIEW_JPEG_QUALITY,
                Path::new(&output_path),
            )
        })
        .await
        .map_err(|e| AppError::ColorGradingError(format!("Blocking task failed: {}", e)))??;

        let url = format!(
            "http://image-preview.localhost/{}",
            percent_encode(&output_path.to_string_lossy())
        );
        Ok(url)
    }
```

- [ ] **Step 2: Register the new module in `mod.rs`**

Add `pub mod preview;` to `src-tauri/src/color_grading/mod.rs`:

```rust
pub mod ffi;
pub mod lensfun_db;
pub mod lut_data;
pub mod presets;
pub mod preview;
pub mod progress;
pub mod resources;
pub mod service;
```

- [ ] **Step 3: Verify compilation**

```bash
cd /mnt/d/GitRepos/CameraFTP/src-tauri && cargo.exe check --target x86_64-pc-windows-msvc
```

Expected: Compiles without errors.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/color_grading/preview.rs src-tauri/src/color_grading/mod.rs
git commit -m "feat(color-grading): add preview session state management"
```

---

### Task 6: Tauri Commands + Registration

**Files:**
- Modify: `src-tauri/src/commands/color_grading.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add 3 new Tauri commands to `commands/color_grading.rs`**

Append at the end of the file (after the `is_raw_file` function):

```rust
use crate::color_grading::preview::ColorGradingPreviewState;

#[command]
pub async fn begin_color_grading_preview(
    preview: State<'_, ColorGradingPreviewState>,
    image_path: String,
) -> Result<(), AppError> {
    let lensfun_db_path = crate::color_grading::resources::get_resources()
        .ok()
        .map(|r| r.lensfun_db_dir.to_string_lossy().into_owned());
    preview.begin(&image_path, lensfun_db_path.as_deref()).await
}

#[command]
pub async fn apply_color_grading_preview(
    preview: State<'_, ColorGradingPreviewState>,
    lut_id: String,
    use_auto_exposure: bool,
    metering_mode: String,
    manual_ev: f32,
) -> Result<String, AppError> {
    preview.apply(&lut_id, use_auto_exposure, &metering_mode, manual_ev).await
}

#[command]
pub async fn end_color_grading_preview(
    preview: State<'_, ColorGradingPreviewState>,
) -> Result<(), AppError> {
    preview.end().await
}
```

- [ ] **Step 2: Register commands and state in `lib.rs`**

In the `use commands::{...}` block (lines 33-74), add the 3 new commands:

```rust
    begin_color_grading_preview,
    apply_color_grading_preview,
    end_color_grading_preview,
```

In the `setup` closure, after managing `ColorGradingService` (after line 196), add:

```rust
            app.manage(color_grading::preview::ColorGradingPreviewState::new());
```

In the `invoke_handler` macro (lines 221-284), add the 3 new commands after `cancel_color_grading`:

```rust
            begin_color_grading_preview,
            apply_color_grading_preview,
            end_color_grading_preview,
```

- [ ] **Step 3: Verify compilation**

```bash
cd /mnt/d/GitRepos/CameraFTP/src-tauri && cargo.exe check --target x86_64-pc-windows-msvc
```

Expected: Compiles without errors.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/color_grading.rs src-tauri/src/lib.rs
git commit -m "feat(color-grading): add preview Tauri commands"
```

---

### Task 7: Full Build Verification

- [ ] **Step 1: Run the full build**

```bash
cd /mnt/d/GitRepos/CameraFTP && ./build.sh windows
```

Expected: All builds complete successfully (C++ DLL + Rust frontend + Windows binary).

- [ ] **Step 2: Run tests**

```bash
cd /mnt/d/GitRepos/CameraFTP/src-tauri && cargo.exe test --target x86_64-pc-windows-msvc
```

Expected: All existing tests pass. No regressions.
