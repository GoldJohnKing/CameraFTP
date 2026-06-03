# Granular Preview Controls — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add independent lens correction toggle to the preview session pipeline, allowing callers to switch lens correction on/off without restarting the session.

**Architecture:** C++ session caches two buffers (decoded RAW + lens-corrected, lazy-loaded). A new `raToggleLensCorrection` C API function switches the active buffer. Rust FFI exposes this through the existing `apply_color_grading_preview` Tauri command via a new `enable_lens_correction` parameter.

**Tech Stack:** C++ (RawAlchemyCpp), Rust (FFI via libloading), Tauri v2 commands

**Spec:** `docs/superpowers/specs/2026-06-03-granular-preview-controls-design.md`

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `src-tauri/lib/rawalchemy/src/raw_alchemy_capi.cpp` | Modify | Session struct, `raBeginPreviewSession` refactoring, `raToggleLensCorrection`, `raApplyPreviewGrading` source selection |
| `src-tauri/lib/rawalchemy/include/raw_alchemy_capi.h` | Modify | Declare `raToggleLensCorrection` |
| `src-tauri/src/color_grading/ffi.rs` | Modify | New fn pointer type, `toggle_lens_correction` method |
| `src-tauri/src/color_grading/preview.rs` | Modify | `ActiveSession` field, `apply()` parameter + toggle logic |
| `src-tauri/src/commands/color_grading.rs` | Modify | `apply_color_grading_preview` gains `enable_lens_correction` param |

---

### Task 1: C++ — Update session struct and `raBeginPreviewSession`

**Files:**
- Modify: `src-tauri/lib/rawalchemy/src/raw_alchemy_capi.cpp:65-68` (session struct)
- Modify: `src-tauri/lib/rawalchemy/src/raw_alchemy_capi.cpp:615-654` (`raBeginPreviewSession`)

- [ ] **Step 1: Update `RaPreviewSession_` struct**

Replace lines 65-68 with:

```cpp
struct RaPreviewSession_ {
    rawalchemy::ImageBuffer decodedImage;   // raw decode (uncorrected), never modified
    rawalchemy::ImageBuffer correctedImage; // lens-corrected, lazy-loaded (empty until computed)
    std::string inputPath;
    bool useCorrected;                      // selects active buffer for grading
};
```

- [ ] **Step 2: Refactor `raBeginPreviewSession`**

Replace lines 615-654 with:

```cpp
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
        rawalchemy::ImageBuffer correctedImg; // empty by default

        if (enableLensCorrection) {
            try {
                auto meta = rawalchemy::extractMetadata(std::string(inputPath));
                rawalchemy::LensCorrectionParams lcParams;
                lcParams.enabled = true;
                lcParams.correctDistortion = true;
                lcParams.correctTca = true;
                lcParams.correctVignetting = true;
                lcParams.distance = 1000.0f;
                if (customLensfunDb) lcParams.customDbPath = customLensfunDb;
                rawalchemy::applyLensCorrection(img, meta, lcParams);
                correctedImg = img; // cache the corrected result
            } catch (...) {
                return catchExceptions("lens correction");
            }
        }

        *outSession = new RaPreviewSession_{
            std::move(img),               // decodedImage: always the uncorrected raw decode
            std::move(correctedImg),       // correctedImage: populated only if enabled
            std::string(inputPath),
            enableLensCorrection != 0      // useCorrected
        };
        return RA_OK;
    } catch (...) {
        return catchExceptions("raBeginPreviewSession");
    }
}
```

**Important behavioral change:** Previously `decodedImage` held the *corrected* image when lens correction was on. Now `decodedImage` always holds the *uncorrected* raw decode, and `correctedImage` holds the lens-corrected version. When `enableLensCorrection=1`, both are populated — the uncorrected is stored first, then corrected is computed from it. When `enableLensCorrection=0`, only `decodedImage` is populated.

Wait — this has a problem. If `enableLensCorrection=1`, the lens correction mutates `img` in-place, so after correction `img` is corrected, not uncorrected. We need to clone before correction.

Replace the `enableLensCorrection` block with:

```cpp
        if (enableLensCorrection) {
            try {
                auto meta = rawalchemy::extractMetadata(std::string(inputPath));
                // Clone before correction so decodedImage stays uncorrected
                correctedImg = img;
                rawalchemy::LensCorrectionParams lcParams;
                lcParams.enabled = true;
                lcParams.correctDistortion = true;
                lcParams.correctTca = true;
                lcParams.correctVignetting = true;
                lcParams.distance = 1000.0f;
                if (customLensfunDb) lcParams.customDbPath = customLensfunDb;
                rawalchemy::applyLensCorrection(correctedImg, meta, lcParams);
            } catch (...) {
                return catchExceptions("lens correction");
            }
        }
```

This way `img` remains the uncorrected raw decode, and `correctedImg` is a clone with lens correction applied.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/lib/rawalchemy/src/raw_alchemy_capi.cpp
git commit -m "refactor: update RaPreviewSession_ struct for dual-buffer lens correction"
```

---

### Task 2: C++ — Implement `raToggleLensCorrection` and update `raApplyPreviewGrading`

**Files:**
- Modify: `src-tauri/lib/rawalchemy/src/raw_alchemy_capi.cpp:656-716` (`raApplyPreviewGrading`, `raEndPreviewSession`)
- Modify: `src-tauri/lib/rawalchemy/include/raw_alchemy_capi.h:227-229` (declare new function)

- [ ] **Step 1: Add `raToggleLensCorrection` declaration to header**

Insert before the `raEndPreviewSession` declaration (before line 227 in `raw_alchemy_capi.h`):

```c
/** Toggle lens correction for an existing preview session.
 *
 *  If enabling and the corrected buffer has not been computed yet,
 *  lens correction is applied to a clone of the decoded image and cached
 *  (lazy-loaded, ~200-500ms on first call).
 *  If the corrected buffer is already cached, switching is instantaneous.
 *  Disabling is always instantaneous.
 *
 *  @param session         Active preview session.
 *  @param enable          If non-zero, use lens-corrected buffer.
 *  @param customLensfunDb Custom Lensfun DB path, or NULL.
 *  @return RA_OK on success. */
RA_API RaResult RA_CALL raToggleLensCorrection(
    RaPreviewSession session,
    int              enable,
    const char*      customLensfunDb
);
```

- [ ] **Step 2: Implement `raToggleLensCorrection` in cpp**

Insert after `raBeginPreviewSession` (after line 654) and before `raApplyPreviewGrading`:

```cpp
RA_API RaResult RA_CALL raToggleLensCorrection(
    RaPreviewSession session,
    int              enable,
    const char*      customLensfunDb)
{
    if (!session) {
        setError("raToggleLensCorrection: null session");
        return RA_ERR_INVALID_PARAM;
    }
    clearError();

    if (enable) {
        session->useCorrected = true;
        // Lazy-load: compute lens correction if not done yet
        if (session->correctedImage.width == 0) {
            try {
                auto meta = rawalchemy::extractMetadata(session->inputPath);
                session->correctedImage = session->decodedImage;
                rawalchemy::LensCorrectionParams lcParams;
                lcParams.enabled = true;
                lcParams.correctDistortion = true;
                lcParams.correctTca = true;
                lcParams.correctVignetting = true;
                lcParams.distance = 1000.0f;
                if (customLensfunDb) lcParams.customDbPath = customLensfunDb;
                rawalchemy::applyLensCorrection(session->correctedImage, meta, lcParams);
            } catch (...) {
                session->useCorrected = false;
                return catchExceptions("lens correction");
            }
        }
    } else {
        session->useCorrected = false;
    }

    return RA_OK;
}
```

- [ ] **Step 3: Update `raApplyPreviewGrading` source selection**

Replace line 677 (`auto img = session->decodedImage;`) with:

```cpp
        // Select source buffer based on lens correction state
        auto& source = session->useCorrected ? session->correctedImage : session->decodedImage;
        auto img = source;
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/lib/rawalchemy/src/raw_alchemy_capi.cpp src-tauri/lib/rawalchemy/include/raw_alchemy_capi.h
git commit -m "feat: add raToggleLensCorrection and dual-buffer grading"
```

---

### Task 3: Rust FFI — Add `toggle_lens_correction` binding

**Files:**
- Modify: `src-tauri/src/color_grading/ffi.rs`

- [ ] **Step 1: Add function pointer type**

Insert after the `RaEndPreviewSessionFn` type alias (after line 182):

```rust
type RaToggleLensCorrectionFn = unsafe extern "C" fn(
    *mut std::ffi::c_void, // session
    c_int,                 // enable (0/1)
    *const c_char,         // customLensfunDb (nullable)
) -> c_int;
```

- [ ] **Step 2: Add field to `RawAlchemyLib` struct**

Add `toggle_lens_correction` field to the struct (after line 191):

```rust
pub struct RawAlchemyLib {
    _lib: Library,
    process_file_with_lut: RaProcessFileWithLUTFn,
    get_last_error: RaGetLastErrorFn,
    get_version: RaGetVersionFn,
    begin_preview_session: RaBeginPreviewSessionFn,
    apply_preview_grading: RaApplyPreviewGradingFn,
    end_preview_session: RaEndPreviewSessionFn,
    toggle_lens_correction: RaToggleLensCorrectionFn,
}
```

- [ ] **Step 3: Load symbol in `load()` method**

Insert after the `end_preview_session` symbol loading block (after line 267), before the `Ok(Self { ... })`:

```rust
        let toggle_lens_correction = unsafe {
            *lib.get::<RaToggleLensCorrectionFn>(b"raToggleLensCorrection\0")
                .map_err(|e| {
                    AppError::ColorGradingError(format!(
                        "Symbol raToggleLensCorrection not found: {}",
                        e
                    ))
                })?
        };
```

Add the field to the struct initialization:

```rust
        Ok(Self {
            _lib: lib,
            process_file_with_lut,
            get_last_error,
            get_version,
            begin_preview_session,
            apply_preview_grading,
            end_preview_session,
            toggle_lens_correction,
        })
```

- [ ] **Step 4: Add wrapper method**

Insert before `end_preview_session` method (before line 464):

```rust
    pub(crate) fn toggle_lens_correction(
        &self,
        session: &RaPreviewSession,
        enable: bool,
        lensfun_db_path: Option<&str>,
    ) -> Result<(), AppError> {
        let lensfun_c = lensfun_db_path
            .map(|s| std::ffi::CString::new(s).map_err(|e| AppError::ColorGradingError(format!("Invalid lensfun path: {}", e))))
            .transpose()?;

        let result = unsafe {
            (self.toggle_lens_correction)(
                session.ptr,
                if enable { 1 } else { 0 },
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
```

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/color_grading/ffi.rs
git commit -m "feat: add raToggleLensCorrection FFI binding"
```

---

### Task 4: Rust — Update `preview.rs` with toggle logic

**Files:**
- Modify: `src-tauri/src/color_grading/preview.rs`

- [ ] **Step 1: Add `enable_lens_correction` field to `ActiveSession`**

Replace lines 16-20 with:

```rust
struct ActiveSession {
    session: RaPreviewSession,
    image_path: String,
    preview_output_path: PathBuf,
    enable_lens_correction: bool,
}
```

- [ ] **Step 2: Update `begin()` to record initial state**

In the `begin()` method, update the `ActiveSession` construction (around line 72-76). The current code is:

```rust
        *guard = Some(ActiveSession {
            session,
            image_path: image_path.to_string(),
            preview_output_path,
        });
```

Replace with:

```rust
        *guard = Some(ActiveSession {
            session,
            image_path: image_path.to_string(),
            preview_output_path,
            enable_lens_correction: true,
        });
```

Note: `begin()` always passes `true` for lens correction currently (line 54-56). The `enable_lens_correction` field records this initial state.

- [ ] **Step 3: Update `apply()` signature and add toggle logic**

Replace the entire `apply()` method (lines 81-127) with:

```rust
    pub async fn apply(
        &self,
        lut_id: &str,
        enable_lens_correction: bool,
        use_auto_exposure: bool,
        metering_mode: &str,
        manual_ev: f32,
    ) -> Result<String, AppError> {
        let lib = RawAlchemyLib::get()?;
        let preset = find_preset(lut_id)
            .ok_or_else(|| AppError::ColorGradingError(format!("Unknown LUT preset: {}", lut_id)))?;
        let lut_data = lut_data::get_lut_data(&preset.id)?;

        let lensfun_db_path = super::resources::get_resources()
            .ok()
            .map(|r| r.lensfun_db_dir.to_string_lossy().into_owned());

        let (session_addr, output_path) = {
            let mut guard = self.inner.lock().await;
            let active = guard.as_mut()
                .ok_or_else(|| AppError::ColorGradingError("No active preview session".into()))?;

            // Toggle lens correction if state changed
            if enable_lens_correction != active.enable_lens_correction {
                tracing::info!(
                    from = active.enable_lens_correction,
                    to = enable_lens_correction,
                    "Toggling lens correction"
                );
                let session = RaPreviewSession { ptr: active.session.ptr };
                lib.toggle_lens_correction(&session, enable_lens_correction, lensfun_db_path.as_deref())?;
                active.enable_lens_correction = enable_lens_correction;
            }

            (active.session.ptr as usize, active.preview_output_path.clone())
        };
        let output_path_for_url = output_path.clone();

        let log_space = preset.log_space.clone();
        let metering = metering_mode.to_string();

        tracing::debug!(lut = lut_id, ev = manual_ev, lens = enable_lens_correction, "Applying preview grading");

        tokio::task::spawn_blocking(move || {
            let session = RaPreviewSession { ptr: session_addr as *mut std::ffi::c_void };
            lib.apply_preview_grading(
                &session,
                Some(log_space.as_str()),
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
            percent_encode(&output_path_for_url.to_string_lossy())
        );
        Ok(url)
    }
```

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/color_grading/preview.rs
git commit -m "feat: add lens correction toggle to preview apply"
```

---

### Task 5: Tauri command — Add `enable_lens_correction` parameter

**Files:**
- Modify: `src-tauri/src/commands/color_grading.rs:62-71`

- [ ] **Step 1: Update `apply_color_grading_preview` command**

Replace lines 62-71 with:

```rust
#[command]
pub async fn apply_color_grading_preview(
    preview: State<'_, ColorGradingPreviewState>,
    lut_id: String,
    enable_lens_correction: bool,
    use_auto_exposure: bool,
    metering_mode: String,
    manual_ev: f32,
) -> Result<String, AppError> {
    preview.apply(&lut_id, enable_lens_correction, use_auto_exposure, &metering_mode, manual_ev).await
}
```

- [ ] **Step 2: Build to verify compilation**

Run: `./build.sh windows android`

Expected: Build succeeds with no errors. The Tauri command registration in `lib.rs` does not need changes — it matches by name and the new parameter is automatically handled.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/commands/color_grading.rs
git commit -m "feat: add enable_lens_correction param to apply_color_grading_preview"
```
