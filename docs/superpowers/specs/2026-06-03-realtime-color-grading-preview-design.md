# Real-time Color Grading Preview

## Summary

Add a two-phase preview pipeline that caches the decoded RAW image in C++,
allowing subsequent LUT/exposure parameter changes to produce preview JPEGs
in ~150-200ms instead of the current 2-5s full pipeline.

Scope: C++ library API, Rust FFI layer, and Tauri commands only.
UI integration will be a separate round.

## Motivation

Currently, color grading is a batch operation: user selects LUT + exposure,
clicks "Apply", and waits 2-5 seconds for the full RAW decode + grading
pipeline. There is no preview mechanism — users cannot see the effect of
different LUTs or exposure values before committing.

The full pipeline is:

```
Decode RAW → Lens Correction → Exposure → Sat/Contrast → Log Transform → LUT → JPEG Encode
```

The slow steps are Decode RAW + Lens Correction (~2-5s). The remaining steps
(Exposure → JPEG) take only ~50-100ms. By caching the post-lens-correction
image, we can re-run just the fast steps on each parameter change.

## Design

### Architecture Decision: C++ Owns Cached Data

The decoded pixel data is created, consumed, and stored entirely in C++ heap
memory. C++ provides a session-based API that encapsulates both the cached
data and its lifecycle. Rust acts as the orchestrator, calling session
functions at the correct business-event timing.

Rationale: the data's full lifecycle (create → store → consume → destroy)
lives within C++ capability. Rust only needs to know *when* to trigger each
operation, not *how* the data is stored. This gives clean separation:
C++ owns data, Rust owns orchestration.

### C++ API

New types and functions added to `raw_alchemy_capi.h`:

```c
typedef struct RaPreviewSession_* RaPreviewSession;

RA_API RaResult RA_CALL raBeginPreviewSession(
    const char* inputPath,
    int         enableLensCorrection,
    const char* customLensfunDb,
    RaPreviewSession* outSession
);

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

RA_API void RA_CALL raEndPreviewSession(RaPreviewSession session);
```

#### raBeginPreviewSession

Pipeline: `decodeRaw → extractMetadata → applyLensCorrection → cache`

Stores the post-lens-correction `ImageBuffer` in the session struct.
This is the slow step (~2-5s), called once when the user opens the
color grading dialog.

#### raApplyPreviewGrading

Pipeline on a **cloned** copy of the cached data:

`clone(decodedImage) → applyGain → applySaturationContrast → applyLogEncoding → applyLUT3D → writeJpeg`

The original cached data is never modified. Safe to call repeatedly with
different parameters. Each call produces a JPEG at `outputPath`.

Clone cost: ~274MB for 6000×4000 image = ~50-80ms memcpy.
Total per-preview: ~150-200ms.

JPEG quality for preview: 80 (vs 95 for full export) to reduce encode time.

#### raEndPreviewSession

Frees the cached `ImageBuffer` and session memory. Safe on NULL (no-op).

### Rust FFI Layer

#### New Symbol Types (`ffi.rs`)

```rust
type RaBeginPreviewSessionFn = unsafe extern "C" fn(
    *const c_char, *const c_char, *mut RaPreviewSession,
) -> c_int;

type RaApplyPreviewGradingFn = unsafe extern "C" fn(
    RaPreviewSession, *const c_char, *const c_float, c_int,
    *const c_float, *const c_float, *const c_char,
    c_float, c_int, c_int, *const c_char,
) -> c_int;

type RaEndPreviewSessionFn = unsafe extern "C" fn(RaPreviewSession);
```

#### Session Handle Wrapper

```rust
#[repr(transparent)]
struct RaPreviewSession { ptr: *mut std::ffi::c_void }
```

Three new methods on `RawAlchemyLib`:
- `begin_preview_session(&self, input_path, ...) -> Result<RaPreviewSession, AppError>`
- `apply_preview_grading(&self, session, lut_data, ...) -> Result<(), AppError>`
- `end_preview_session(&self, session)`

### Rust State Management

New module: `color_grading/preview.rs`

```rust
pub struct ColorGradingPreviewState {
    inner: Mutex<Option<ActivePreviewSession>>,
}

struct ActivePreviewSession {
    session: RaPreviewSession,
    image_path: String,
    preview_output_path: PathBuf,
}
```

Rules:
- One active session at a time (enforced by `Mutex<Option<...>>`)
- `begin` evicts existing session before creating new one
- `apply` returns error if no active session
- `end` frees C++ session + deletes temp JPEG file

### Tauri Commands

Three new commands in `commands/color_grading.rs`:

```rust
#[command]
async fn begin_color_grading_preview(
    state: State<'_, ColorGradingPreviewState>,
    image_path: String,
) -> Result<(), AppError>

#[command]
async fn apply_color_grading_preview(
    state: State<'_, ColorGradingPreviewState>,
    lut_id: String,
    use_auto_exposure: bool,
    metering_mode: String,
    manual_ev: f32,
) -> Result<String, AppError>  // Returns preview image URL

#[command]
async fn end_color_grading_preview(
    state: State<'_, ColorGradingPreviewState>,
) -> Result<(), AppError>
```

`apply_color_grading_preview` internal flow:
1. Lock state → get `ActivePreviewSession`
2. `find_preset(lut_id)` → logSpace + metadata
3. `get_lut_data(preset_id)` → LUT data (cached in existing DashMap)
4. `spawn_blocking` → `lib.apply_preview_grading(session, ...)`
5. Return URL: `http://image-preview.localhost/{encoded_preview_path}`

### Calling Flow (for future UI integration)

```
Dialog opens  → begin_color_grading_preview({ imagePath })
Param change  → apply_color_grading_preview({ lutId, exposure... })  // debounced
Param change  → apply_color_grading_preview({ lutId, exposure... })  // debounced
Dialog cancel → end_color_grading_preview()
Dialog apply  → end_color_grading_preview() + enqueue_color_grading(...)
```

## Files Changed

| File | Change |
|------|--------|
| `src-tauri/lib/rawalchemy/include/raw_alchemy_capi.h` | Add session type + 3 API declarations |
| `src-tauri/lib/rawalchemy/src/raw_alchemy_capi.cpp` | Implement session struct + 3 API functions |
| `src-tauri/src/color_grading/ffi.rs` | Add 3 symbol types + load + wrapper methods |
| `src-tauri/src/color_grading/preview.rs` | **New**: preview state management |
| `src-tauri/src/color_grading/mod.rs` | Add `pub mod preview` |
| `src-tauri/src/commands/color_grading.rs` | Add 3 Tauri commands |
| `src-tauri/src/lib.rs` | Register commands + manage state |

Not changed: frontend components, hooks, existing batch processing pipeline.

## Performance Targets

| Operation | Expected Time |
|-----------|--------------|
| `begin_color_grading_preview` | 2-5s (one-time RAW decode) |
| `apply_color_grading_preview` | 150-200ms (clone + grade + encode) |
| `end_color_grading_preview` | <1ms |

## Future Work (separate round)

- ColorGradingDialog: add `onSettingsChange` callback with debounce
- PreviewWindow: switch image source to preview result on param change
- Cancel: revert to original preview image
- Loading indicator during initial decode
