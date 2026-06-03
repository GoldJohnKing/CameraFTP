# Granular Preview Controls — Independent Lens Correction Toggle

**Date:** 2026-06-03
**Status:** Approved
**Scope:** C++ / Rust FFI / Tauri commands only (no UI changes)

## Background

The preview session pipeline (`begin → apply → end`) caches a decoded RAW image in C++ heap memory. Currently, lens correction is applied once during `begin` and baked into the cached buffer — it cannot be changed without restarting the session.

## Goal

Allow the caller to toggle lens correction on/off during preview without restarting the session. The corrected buffer is lazy-loaded: only computed when first requested.

## Scope

| Layer | Changes | No changes |
|-------|---------|------------|
| C++ (`raw_alchemy_capi.cpp/.h`) | Session struct, new `raToggleLensCorrection`, `raApplyPreviewGrading` behavior | `raProcessFile*`, `raBeginPreviewSession` signature |
| Rust FFI (`ffi.rs`) | New fn pointer + wrapper method | `begin_preview_session`, `end_preview_session` signatures |
| Rust preview (`preview.rs`) | `apply()` gains `enable_lens_correction` param, toggle logic | `begin()`, `end()` signatures |
| Tauri commands (`commands/color_grading.rs`) | `apply_color_grading_preview` gains `enable_lens_correction` param | All other commands |
| Frontend | None (future work) | `ColorGradingDialog`, config types |

## Design

### C++: Session Struct

```cpp
struct RaPreviewSession_ {
    rawalchemy::ImageBuffer decodedImage;   // raw decode, never modified
    rawalchemy::ImageBuffer correctedImage; // lens-corrected, lazy-loaded (empty until computed)
    std::string inputPath;
    bool useCorrected;                      // active buffer selector
};
```

No separate `correctedReady` flag — `correctedImage` starts empty (width=0, data=nullptr) and is considered ready when non-empty (`correctedImage.width > 0`).

Memory: ~576 MB for a 24MP image (2 × 288 MB float32 buffers). The second buffer is only allocated on first toggle to lens correction enabled.

### C++: `raBeginPreviewSession` (signature unchanged)

Behavior depends on `enableLensCorrection`:

- `enableLensCorrection=1`: decode RAW → cache as `decodedImage`, apply lens correction → cache as `correctedImage`, set `useCorrected=true`.
- `enableLensCorrection=0`: decode RAW → cache as `decodedImage`, leave `correctedImage` empty, set `useCorrected=false`.

### C++: `raToggleLensCorrection` (new)

```c
RA_API RaResult RA_CALL raToggleLensCorrection(
    RaPreviewSession session,
    int              enable,
    const char*      customLensfunDb
);
```

Logic:

| `enable` | `correctedImage` state | Action |
|----------|----------------------|--------|
| 1 | empty | Compute lens correction from `decodedImage` → cache to `correctedImage` (~200-500ms). Set `useCorrected=true`. |
| 1 | non-empty | Set `useCorrected=true` only (~0ms). |
| 0 | any | Set `useCorrected=false` only (~0ms). |

### C++: `raApplyPreviewGrading` (behavior change)

Source buffer selection changes:

```
activeBuffer = useCorrected ? correctedImage : decodedImage
clone activeBuffer → grading pipeline → write JPEG
```

Signature is unchanged. The caller does not pass `enableLensCorrection` here — the session's internal `useCorrected` flag determines the source.

### Rust: FFI (`ffi.rs`)

New function pointer type:

```rust
type RaToggleLensCorrectionFn = unsafe extern "C" fn(
    *mut std::ffi::c_void, // session
    c_int,                 // enable
    *const c_char,         // customLensfunDb (nullable)
) -> c_int;
```

`RawAlchemyLib` gains `toggle_lens_correction` field, loaded from `raToggleLensCorrection` symbol.

New wrapper method:

```rust
pub(crate) fn toggle_lens_correction(
    &self,
    session: &RaPreviewSession,
    enable: bool,
    lensfun_db_path: Option<&str>,
) -> Result<(), AppError>
```

### Rust: Preview State (`preview.rs`)

`ActiveSession` gains a field:

```rust
struct ActiveSession {
    session: RaPreviewSession,
    image_path: String,
    preview_output_path: PathBuf,
    enable_lens_correction: bool,
}
```

`begin()`: records the initial `enable_lens_correction` state into `ActiveSession`.

`apply()`: signature gains `enable_lens_correction: bool` parameter. Before cloning + grading:

```
if enable_lens_correction != active.enable_lens_correction:
    lib.toggle_lens_correction(&session, enable_lens_correction, lensfun_db_path)
    active.enable_lens_correction = enable_lens_correction
```

`end()`: unchanged.

### Tauri Command

```rust
#[command]
pub async fn apply_color_grading_preview(
    preview: State<'_, ColorGradingPreviewState>,
    lut_id: String,
    enable_lens_correction: bool,     // NEW
    use_auto_exposure: bool,
    metering_mode: String,
    manual_ev: f32,
) -> Result<String, AppError>
```

Registered in `lib.rs` invoke handler (already present, signature change is transparent to Tauri).

## Performance Characteristics

| Operation | Latency | Notes |
|-----------|---------|-------|
| `begin` with lens correction | ~2-5s | Decode + lens correction |
| `begin` without lens correction | ~2-5s | Decode only |
| `apply` (no lens correction change) | ~150ms | Clone + grading |
| `apply` (toggle lens correction ON, first time) | ~200-500ms + ~150ms | Lazy lens correction + grading |
| `apply` (toggle lens correction ON, subsequent) | ~150ms | Already cached |
| `apply` (toggle lens correction OFF) | ~150ms | Just switches source buffer |

## Backward Compatibility

- `begin_color_grading_preview`: no signature change. Frontend continues to call it as before.
- `apply_color_grading_preview`: gains `enable_lens_correction` param. Until UI is updated, frontend passes `true` to maintain current behavior.
- `end_color_grading_preview`: no change.
- Batch pipeline (`enqueue_color_grading`): no change.
- Config types: no change.

## Out of Scope (Future Work)

- UI toggle for lens correction in `ColorGradingDialog`
- Persisting `enable_lens_correction` in `ColorGradingLastUsed` / `AutoColorGradingConfig`
- Passing `enable_lens_correction` through the batch pipeline
- Debounced `onSettingsChange` callback for real-time preview updates
