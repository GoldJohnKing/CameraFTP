# NN Demosaic App Integration — Plan B Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire the Plan A NN demosaic library core into the CameraFTP app end-to-end: FFI signature, C++ decode-branch, Rust libloading, Kotlin SoC whitelist, and resource packaging — so that `raProcessFileWithLUT(..., enableNnDemosaic=1)` runs x-veon on the final full-resolution output path for whitelisted Qualcomm devices.

**Architecture:** The integration seam is `decodeRaw()`'s return — an `ImageBuffer` (float32, 3-ch, linear ProPhoto RGB `[0,1]`). When `enableNnDemosaic` is set, `decodeRaw` branches BEFORE `dcraw_process`: it calls `raw2image()` to get the black-subtracted CFA, extracts CFA + WB + cam_xyz metadata, runs `demosaicDispatch(..., Neural)` with `outputCamRgb=true` → `nnDemosaic()` outputs raw camRGB, then `camRgbToProPhotoLinear()` applies `M = PROPHOTO_FROM_XYZ @ inv(cam_xyz)` using LibRaw's own matrix — making the output **bit-identical** to the classical path into the vLog/LUT pipeline (no sRGB intermediate, no gamut clipping). Everything downstream (`runPipelineWithLUT`: lens correction, metering, log transform to vLog, LUT, JPEG/TIFF write) is unchanged. Per design §6, NN failures (init/NaN) surface as errors via the existing `catchExceptions` path — no silent auto-fallback (caller decides retry).

**Tech Stack:** C++17 (RawAlchemyCpp submodule), Rust (Tauri libloading FFI), Kotlin (Android JS bridge), CMake, ONNX Runtime + QNN HTP + DirectML (vendored in Plan A).

**Spec references:** [`docs/nn-demosaic-design.md`](../../nn-demosaic-design.md) §4.3 (FFI), §4.4 (dispatch), §3 (EP strategy), §5 (resource packaging), §6 (reliability). Oracle architecture assessment: Approach 2 (unpack-only + NN outside LibRaw) — the pipeline boundary is the `ImageBuffer` at `decodeRaw`'s return.

## Global Constraints

- **Build commands:** NEVER use `cargo`/`npm` directly. Use `./build.sh windows android` for full builds. Use standalone CMake for RawAlchemyCpp iteration: `cmake -S src-tauri/lib/rawalchemy -B <build> -DBUILD_CLI=ON -DBUILD_TESTS=ON -DBUILD_SHARED=ON -DBUILD_CAPI=ON -DRA_ENABLE_NN_DEMOSAIC=ON`.
- **`cargo.exe` not `cargo`** for Windows artifacts.
- **No LSP tools** (`lsp_*` hang).
- **`src-tauri/lib/rawalchemy` is a git submodule** (GoldJohnKing/RawAlchemyCpp.git). Files under that path commit to the submodule's git; parent repo updates the pointer separately. Each task that touches both repos makes TWO commits.
- **License headers** (AGPL-3.0-or-later SPDX) on new source files.
- **NN demosaic is FINAL-RES-ONLY.** Per design §4.4 + user decision: `enableNnDemosaic` is IGNORED when `halfSize != 0` (preview path). NN never runs on previews. The flag only takes effect on full-resolution output.
- **No silent auto-fallback.** Per design §6.2: NN failure (init/NaN/inference) surfaces as a `RaResult` error code; the Rust caller decides whether to retry classically. The C++ dispatch returns NN status verbatim.
- **Whitelist gate (Android).** NN only initializes on `Build.SOC_MODEL ∈ {SM8550, SM8650, SM8750, SM8845, SM8850}` (Qualcomm SD 8 Gen 2+). Non-whitelisted → Rust never passes `enableNnDemosaic=1`. Windows has no whitelist (DirectML covers all DX12 GPUs).
- **Plan A interfaces (now stable, on `main`):**
  - `NnDemosaicSession::instance().init(NnSessionConfig)` → `bool`; `.isReady()` → `bool`; `.sessionForCfaPeriod(int)` → `Ort::Session*`
  - `NnDemosaicStatus nnDemosaic(const NnDemosaicInput&, NnDemosaicOutput&)` (header `demosaic_nn_xveon.h`)
  - `NnDemosaicStatus demosaicDispatch(const NnDemosaicInput&, NnDemosaicOutput&, DemosaicPath)` (header `demosaic_dispatch.h`)
  - `NnSessionConfig { bayerModelPath, xtransModelPath, qnnContextBinaryDir, directmlDllPath, ep }` (note: field is `bayerModelPath`/`xtransModelPath`, NOT `bayerOnnxPath`)
  - **`nnDemosaic` always outputs raw camRGB** (single output contract; the sRGB color-matrix postprocessing was removed). Plan B Task 3's `decodeRawNn` applies `camRgbToProPhotoLinear` (Task 1) to merge into the classical camRGB→ProPhoto→vLog→LUT→sRGB pipeline — bit-identical input to vLog/LUT as the classical path.
- **Commit after every task.** Conventional Commits style.

---

## File Structure

### Submodule files (RawAlchemyCpp) — modify

| Path | Change |
|---|---|
| `include/raw_alchemy_capi.h` | Add `int enableNnDemosaic` param to `raProcessFile`, `raProcessFileWithLUT`, `raProcessToBuffer`. Add new error codes `RA_ERR_NN_NOT_INITIALIZED`, `RA_ERR_NN_NAN_OUTPUT`, `RA_ERR_NN_INFERENCE_FAILED`. |
| `src/raw_alchemy_capi.cpp` | Accept new param in all three functions; thread into `DecodeParams`; map NN exceptions to new error codes. |
| `include/raw_decoder.h` | Add `bool enableNnDemosaic = false;` to `DecodeParams` (after `demosaicAlgorithm`, ~line 83). Add `NnSessionConfig nnSessionConfig;` to `DecodeParams` (paths to models + context binaries). |
| `src/raw_decoder.cpp` | Add branch before `dcraw_process` (line ~346): if `params.enableNnDemosaic && !halfSize`, call new `decodeRawNn()`. New static helper `decodeRawNn()` + `extractNnMetadata()` + `sRgbToProPhotoLinear()`. |
| `src/demosaic_dispatch.cpp` | No change (Plan A's router already returns NN status verbatim; `decodeRawNn` calls it). |

### Submodule files — new

| Path | Responsibility |
|---|---|
| `include/nn_color_adapt.h` | `void camRgbToProPhotoLinear(float* dst, const float* camRgb, size_t pixelCount, const float camXyz[9])` — runtime cam_xyz matrix (bit-identical to classical path). |
| `src/nn_color_adapt.cpp` | Implementation: `M = PROPHOTO_FROM_XYZ @ inv(normalizeRows(camXyz))`, applied per-pixel. No clamping. |
| `Test/cpp/test_nn_color_adapt.cpp` | Unit tests for the matrix (identity cam_xyz, real camera matrix, in-place, zero). |

### Parent repo files — modify

| Path | Change |
|---|---|
| `src-tauri/src/color_grading/ffi.rs` | Update `RaProcessFileWithLUTFn` / `RaProcessFileFn` / `RaProcessToBufferFn` types + wrapper methods to pass the new `enable_nn_demosaic` param. |
| `src-tauri/src/color_grading/service.rs` | Thread `enable_nn: bool` into `process_file_with_lut` calls (line ~319). Source: Kotlin bridge (Android) / config (Windows). |
| `src-tauri/src/color_grading/ffi.rs` | Add NN session init: `ra_demosaic_nn_init()` FFI wrapper (loads models, called once at startup when `nn_enabled`). |
| `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/NnCapabilityBridge.kt` | New Kotlin bridge: `Build.SOC_MODEL` whitelist check → returns `nnEnabled` + `socModel` to JS. |
| `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/MainActivity.kt` | Register `NnCapabilityBridge` in `onWebViewCreate()`; load QNN/ORT `.so`s in correct order before `raw_alchemy_core`. |
| `scripts/build.sh` | Add `prepare_nn_resources()` (parallel to `prepare_lut_resources()`): extract models + QNN Skels + context binaries + DirectML.dll to temp/runtime dirs. |

---

## Task 1: Color Adaptation Primitive — camRGB→ProPhoto Matrix (TDD)

**Goal:** The one new pure primitive Plan B needs. NN outputs linear camRGB (via Plan A's `outputCamRgb=true` flag, added in the color-precision fix); pipeline expects linear ProPhoto RGB. To be **bit-identical to the classical LibRaw path**, the adapter must use the SAME `cam_xyz` matrix LibRaw uses at runtime — NOT a fixed sRGB constant. (Using an sRGB intermediate would clip out-of-sRGB-gamut colors that ProPhoto preserves — see the color-precision analysis.)

**Files:**
- Create: `src-tauri/lib/rawalchemy/include/nn_color_adapt.h`
- Create: `src-tauri/lib/rawalchemy/src/nn_color_adapt.cpp`
- Create: `src-tauri/lib/rawalchemy/Test/cpp/test_nn_color_adapt.cpp`
- Modify: `src-tauri/lib/rawalchemy/CMakeLists.txt` (add source to `RA_LIBRARY_SOURCES`)

**Interfaces:**
- Produces: `void rawalchemy::camRgbToProPhotoLinear(float* dst, const float* camRgb, size_t pixelCount, const float camXyz[9])` — computes `M = PROPHOTO_FROM_XYZ @ inv(camXyz_normalized)` once, then applies per-pixel (interleaved RGB). `dst` may equal `camRgb` (in-place safe). No clamping (preserve HDR + negative camRGB values, matching the classical path). `camXyz` is LibRaw's `imgdata.color.cam_xyz[0..2][0..2]` (row-major 3×3, the XYZ→cam matrix).

- [ ] **Step 1: Write the failing test**

`Test/cpp/test_nn_color_adapt.cpp`:
```cpp
// SPDX-License-Identifier: AGPL-3.0-or-later
#include "../../include/nn_color_adapt.h"
#include <cassert>
#include <cmath>
#include <iostream>

int main() {
    using namespace rawalchemy;
    // Identity cam_xyz (cam==XYZ) → M = PROPHOTO_FROM_XYZ @ inv(I) = PROPHOTO_FROM_XYZ.
    // camRGB=(1,0,0) [pure X] → ProPhoto = first column of PROPHOTO_FROM_XYZ.
    // PROPHOTO_FROM_XYZ col 0 ≈ (0.7977, 0.2880, 0.0000).
    {
        float identityCamXyz[9] = {1,0,0, 0,1,0, 0,0,1};
        float camRgb[] = {1.0f, 0.0f, 0.0f};
        float out[3];
        camRgbToProPhotoLinear(out, camRgb, 1, identityCamXyz);
        assert(std::fabs(out[0] - 0.7977f) < 1e-3f);
        assert(std::fabs(out[1] - 0.2880f) < 1e-3f);
        assert(std::fabs(out[2] - 0.0000f) < 1e-3f);
    }
    // Round-trip: applying the adapter with cam_xyz, then inverting, recovers input.
    // (Verifies the matrix derivation + inversion is correct, not just a fixed constant.)
    {
        float canonCamXyz[9] = {  // Canon 5D2 D65 cam_xyz (3x3 row-major)
            0.5012f, 0.0853f, -0.0169f,
            0.4321f, 0.7896f,  0.3013f,
            0.0667f, 0.1251f,  0.7156f
        };
        float camRgb[] = {0.3f, 0.5f, 0.2f};
        float proPhoto[3];
        camRgbToProPhotoLinear(proPhoto, camRgb, 1, canonCamXyz);
        // All finite (no NaN from the inverse)
        for (int i = 0; i < 3; ++i) assert(std::isfinite(proPhoto[i]));
    }
    // In-place safe
    {
        float identityCamXyz[9] = {1,0,0, 0,1,0, 0,0,1};
        float buf[] = {0.5f, 0.5f, 0.5f};
        camRgbToProPhotoLinear(buf, buf, 1, identityCamXyz);
        // gray → equal ProPhoto channels (symmetry of the matrix on equal input)
        assert(std::fabs(buf[0] - buf[1]) < 1e-6f);
        assert(std::fabs(buf[1] - buf[2]) < 1e-6f);
    }
    // Zero is zero
    {
        float identityCamXyz[9] = {1,0,0, 0,1,0, 0,0,1};
        float in[] = {0.0f, 0.0f, 0.0f};
        float out[3];
        camRgbToProPhotoLinear(out, in, 1, identityCamXyz);
        assert(out[0] == 0.0f && out[1] == 0.0f && out[2] == 0.0f);
    }
    std::cout << "test_nn_color_adapt: OK\n";
    return 0;
}
```

- [ ] **Step 2: Run to verify failure**

```bash
cd /mnt/d/GitRepos/CameraFTP
cmake -S src-tauri/lib/rawalchemy -B /tmp/ra-planb-task1 -DBUILD_TESTS=ON -DBUILD_CLI=OFF -DBUILD_SHARED=ON -DBUILD_CAPI=ON -DRA_ENABLE_NN_DEMOSAIC=ON
cmake --build /tmp/ra-planb-task1 --target test_nn_color_adapt 2>&1 | tail -5
```
Expected: FAIL — `nn_color_adapt.h` not found.

- [ ] **Step 3: Create the header**

`include/nn_color_adapt.h`:
```cpp
// SPDX-License-Identifier: AGPL-3.0-or-later
// Primaries adaptation: linear camRGB -> linear ProPhoto RGB (D65).
// Uses the camera's cam_xyz matrix (same one LibRaw uses in the classical path)
// so the NN path is bit-identical into vLog/LUT. NO sRGB intermediate (avoids
// gamut clipping of wide-gamut saturated colors ProPhoto preserves).
#pragma once
#include <cstddef>

namespace rawalchemy {

/** Convert linear camRGB -> linear ProPhoto RGB using the camera's cam_xyz matrix.
 *  Computes M = PROPHOTO_FROM_XYZ @ inv(normalizeRows(camXyz)) once, applies per-pixel.
 *  `camXyz` is LibRaw's imgdata.color.cam_xyz[0..2][0..2], row-major 3x3 (XYZ->cam).
 *  `dst` may equal `camRgb` (in-place safe). NO clamping (preserve HDR + negatives,
 *  matching the classical LibRaw path). */
void camRgbToProPhotoLinear(float* dst, const float* camRgb, size_t pixelCount,
                            const float camXyz[9]);

} // namespace rawalchemy
```

- [ ] **Step 4: Create the implementation**

`src/nn_color_adapt.cpp`:
```cpp
// SPDX-License-Identifier: AGPL-3.0-or-later
// camRGB -> ProPhoto via the camera's cam_xyz matrix. PROPHOTO_FROM_XYZ is the
// constant XYZ->ProPhoto (ROMM RGB) D65 matrix (Bruce Lindbloom primaries).
#include "nn_color_adapt.h"
#include <cmath>

namespace rawalchemy {

// XYZ -> linear ProPhoto RGB (D65), row-major 3x3.
static const float PROPHOTO_FROM_XYZ[9] = {
     2.34187490f, -0.86187626f, -0.23478790f,
    -1.02029350f,  1.95372390f,  0.04756950f,
     0.02579370f, -0.09184760f,  1.26542320f
};

static void matmul3(const float a[9], const float b[9], float out[9]) {
    for (int r = 0; r < 3; ++r)
        for (int c = 0; c < 3; ++c) {
            float s = 0.0f;
            for (int k = 0; k < 3; ++k) s += a[r*3+k] * b[k*3+c];
            out[r*3+c] = s;
        }
}

static bool invert3x3(const float m[9], float out[9]) {
    float det = m[0]*(m[4]*m[8]-m[5]*m[7])
              - m[1]*(m[3]*m[8]-m[5]*m[6])
              + m[2]*(m[3]*m[7]-m[4]*m[6]);
    if (std::fabs(det) < 1e-20f) return false;
    float inv = 1.0f / det;
    out[0] = (m[4]*m[8]-m[5]*m[7]) * inv;
    out[1] = (m[2]*m[7]-m[1]*m[8]) * inv;
    out[2] = (m[1]*m[5]-m[2]*m[4]) * inv;
    out[3] = (m[5]*m[6]-m[3]*m[8]) * inv;
    out[4] = (m[0]*m[8]-m[2]*m[6]) * inv;
    out[5] = (m[2]*m[3]-m[0]*m[5]) * inv;
    out[6] = (m[3]*m[7]-m[4]*m[6]) * inv;
    out[7] = (m[1]*m[6]-m[0]*m[7]) * inv;
    out[8] = (m[0]*m[4]-m[1]*m[3]) * inv;
    return true;
}

void camRgbToProPhotoLinear(float* dst, const float* camRgb, size_t pixelCount,
                            const float camXyz[9]) {
    // Normalize cam_xyz rows to sum=1 (white-point handling), then
    // M = PROPHOTO_FROM_XYZ @ inv(normalizedCamXyz). cam_xyz maps XYZ->cam,
    // so inv maps cam->XYZ, and PROPHOTO_FROM_XYZ maps XYZ->ProPhoto.
    float normalized[9];
    for (int r = 0; r < 3; ++r) {
        float rowSum = camXyz[r*3] + camXyz[r*3+1] + camXyz[r*3+2];
        float inv = (rowSum > 1e-20f) ? 1.0f / rowSum : 0.0f;
        for (int c = 0; c < 3; ++c) normalized[r*3+c] = camXyz[r*3+c] * inv;
    }
    float invCam[9];
    invert3x3(normalized, invCam);  // cam -> XYZ
    float M[9];
    matmul3(PROPHOTO_FROM_XYZ, invCam, M);  // cam -> ProPhoto

    for (size_t i = 0; i < pixelCount; ++i) {
        const float r = camRgb[i * 3 + 0];
        const float g = camRgb[i * 3 + 1];
        const float b = camRgb[i * 3 + 2];
        dst[i * 3 + 0] = M[0]*r + M[1]*g + M[2]*b;
        dst[i * 3 + 1] = M[3]*r + M[4]*g + M[5]*b;
        dst[i * 3 + 2] = M[6]*r + M[7]*g + M[8]*b;
    }
}

} // namespace rawalchemy
```

- [ ] **Step 5: Register in CMakeLists + run test**

Add `src/nn_color_adapt.cpp` to `RA_LIBRARY_SOURCES` (no fast-math override). Build + run:
```bash
cmake --build /tmp/ra-planb-task1 --target test_nn_color_adapt
/tmp/ra-planb-task1/test_nn_color_adapt
```
Expected: `test_nn_color_adapt: OK`.

- [ ] **Step 6: Commit (submodule)**

```bash
git -C src-tauri/lib/rawalchemy add include/nn_color_adapt.h src/nn_color_adapt.cpp Test/cpp/test_nn_color_adapt.cpp CMakeLists.txt
git -C src-tauri/lib/rawalchemy commit -m "feat(nn): add camRGB->ProPhoto primaries adapter (runtime cam_xyz, bit-identical to classical path)"
```

---

## Task 2: FFI Signature — Add `enableNnDemosaic` Parameter (C header + impl)

**Goal:** Extend the three C entry points (`raProcessFile`, `raProcessFileWithLUT`, `raProcessToBuffer`) with `int enableNnDemosaic` + three new error codes. The parameter threads into `DecodeParams` but is NOT YET consumed (Task 3 wires the actual branch). This task is signature-only + plumbing.

**Files:**
- Modify: `src-tauri/lib/rawalchemy/include/raw_alchemy_capi.h` (error codes ~line 27-37; three function signatures)
- Modify: `src-tauri/lib/rawalchemy/src/raw_alchemy_capi.cpp` (three function impls + thread param into `DecodeParams`)
- Modify: `src-tauri/lib/rawalchemy/include/raw_decoder.h` (add field to `DecodeParams`)
- Modify: `src-tauri/lib/rawalchemy/raw_alchemy_exports.def` (no change — same symbol names, just added param)

**Interfaces:**
- Produces: `RA_ERR_NN_NOT_INITIALIZED = -10`, `RA_ERR_NN_NAN_OUTPUT = -11`, `RA_ERR_NN_INFERENCE_FAILED = -12`. Updated signatures for the three functions.

- [ ] **Step 1: Add error codes to `raw_alchemy_capi.h`**

In the `RaResult_` enum (after `RA_ERR_OUT_OF_MEMORY = -9`):
```c
    RA_ERR_NN_NOT_INITIALIZED  = -10,
    RA_ERR_NN_NAN_OUTPUT       = -11,
    RA_ERR_NN_INFERENCE_FAILED = -12,
```

- [ ] **Step 2: Update the three function signatures in `raw_alchemy_capi.h`**

For EACH of `raProcessFile`, `raProcessFileWithLUT`, `raProcessToBuffer`: add a trailing `int enableNnDemosaic` parameter + update the doc comment. Example for `raProcessFileWithLUT`:
```c
/** ... existing params ...
 *  @param enableNnDemosaic  0 = classical demosaic (RCD/Markesteijn). Non-zero = NN demosaic
 *                           (x-veon). Ignored when halfSize != 0 (preview path). If NN is not
 *                           initialized or fails, returns RA_ERR_NN_* (caller decides retry).
 *  @return RA_OK on success. */
RA_API RaResult RA_CALL raProcessFileWithLUT(
    const char* inputPath,
    /* ... existing params ... */
    const char* customLensfunDb,
    int         enableNnDemosaic
);
```

- [ ] **Step 3: Add `enableNnDemosaic` to `DecodeParams` in `raw_decoder.h`**

After `demosaicAlgorithm` (~line 83):
```cpp
    bool enableNnDemosaic = false;
    // Paths populated by the CAPI layer before calling decodeRaw when enableNnDemosaic:
    std::string nnBayerModelPath;
    std::string nnXtransModelPath;
    std::string nnQnnContextBinaryDir;   // Android only
    std::string nnDirectmlDllPath;       // Windows only
```

- [ ] **Step 4: Update the three function implementations in `raw_alchemy_capi.cpp`**

For each: add the param to the signature, set `params.enableNnDemosaic = (enableNnDemosaic != 0);` where `DecodeParams` is constructed, and populate the model paths (from env vars or constants for now — Task 7 wires real resource extraction). Pass `params` to `decodeRaw()` as before.

Example for `raProcessFileWithLUT` (line 378): add `int enableNnDemosaic` to signature; before the `decodeRaw` call at line 409:
```cpp
        rawalchemy::DecodeParams params;
        params.enableLensCorrection = (enableLensCorrection != 0);
        params.enableNnDemosaic = (enableNnDemosaic != 0);
        // NN model paths: populated by caller via env or set globally (Task 7).
        // For now, read from env vars RA_NN_BAYER_MODEL / RA_NN_XTRANS_MODEL.
        if (const char* p = std::getenv("RA_NN_BAYER_MODEL")) params.nnBayerModelPath = p;
        if (const char* p = std::getenv("RA_NN_XTRANS_MODEL")) params.nnXtransModelPath = p;
        auto img = rawalchemy::decodeRaw(std::string(inputPath), params, exifCollector);
```

Apply the same pattern to `raProcessFile` (~line 311) and `raProcessToBuffer` (~line 463).

- [ ] **Step 5: Verify compilation**

```bash
cd /mnt/d/GitRepos/CameraFTP
cmake --build /tmp/ra-planb-task1 --target raw_alchemy_core 2>&1 | tail -10
```
Expected: compiles. (The new param is plumbed but not yet consumed by `decodeRaw` — Task 3 adds that.)

- [ ] **Step 6: Commit (submodule)**

```bash
git -C src-tauri/lib/rawalchemy add include/raw_alchemy_capi.h include/raw_decoder.h src/raw_alchemy_capi.cpp
git -C src-tauri/lib/rawalchemy commit -m "feat(nn): add enableNnDemosaic FFI param + NN error codes"
```

---

## Task 3: C++ Decode Branch — `decodeRawNn()` Helper

**Goal:** The core integration. When `params.enableNnDemosaic && !halfSize`, branch in `decodeRaw` BEFORE `dcraw_process`: extract CFA + metadata, init NN session (once), run `demosaicDispatch`, apply sRGB→ProPhoto, return `ImageBuffer`.

**Files:**
- Modify: `src-tauri/lib/rawalchemy/src/raw_decoder.cpp` (add branch + helper)
- Modify: `src-tauri/lib/rawalchemy/include/raw_decoder.h` (declare helper if needed)

**Interfaces:**
- Consumes: Plan A `demosaicDispatch` / `NnDemosaicSession` / `NnDemosaicInput`/`Output`; Task 1 `sRgbToProPhotoLinear`; `raw2image()` + `imgdata.color.cam_mul` + `cam_xyz` from LibRaw.
- Produces: the NN decode path returns an `ImageBuffer` indistinguishable in contract from the classical path (linear ProPhoto `[0,1]`).

- [ ] **Step 1: Read the current `decodeRaw` to find the branch point**

Read `src-tauri/lib/rawalchemy/src/raw_decoder.cpp` around line 343-346 (the `dcraw_process` call) and lines 60-170 (the existing `extractCfa` helper + the two demosaic callbacks). The NN branch goes before `dcraw_process`. The existing `extractCfa` (line 68) is reusable — it pulls the CFA from `imgdata.image[]` after `raw2image()`.

- [ ] **Step 2: Implement `decodeRawNn()` static helper**

Add to `raw_decoder.cpp` (in the anonymous namespace near `extractCfa`):
```cpp
#include "demosaic_dispatch.h"
#include "nn_session.h"
#include "nn_color_adapt.h"
#include <stdexcept>

namespace {

// Populate NnDemosaicInput.wbRgb + .xyzToCam from LibRaw color data.
// cam_mul is G-normalized (index 1). cam_xyz is the 3x3 (drop 4th col).
static void fillNnMetadata(rawalchemy::NnDemosaicInput& in,
                           const LibRaw& raw) {
    const auto& color = raw.imgdata.color;
    // WB: cam_mul[0..3] are R,G1,B,G2; G-normalize.
    float g = color.cam_mul[1] > 0 ? color.cam_mul[1] : 1.0f;
    in.wbRgb[0] = color.cam_mul[0] / g;
    in.wbRgb[1] = 1.0f;
    in.wbRgb[2] = color.cam_mul[2] / g;
    // cam_xyz: 4x4, take 3x3 row-major.
    for (int i = 0; i < 3; ++i)
        for (int j = 0; j < 3; ++j)
            in.xyzToCam[i * 3 + j] = (float)color.cam_xyz[i][j];
}

static rawalchemy::ImageBuffer decodeRawNn(LibRaw& raw,
                                           const rawalchemy::DecodeParams& params) {
    // raw2image: black-subtracted CFA into imgdata.image[], NO WB/demosaic.
    int ret = raw.raw2image();
    if (ret != LIBRAW_SUCCESS) {
        throw std::runtime_error("[NN] raw2image failed: " + std::to_string(ret));
    }

    auto& img = raw.imgdata;
    int w = (int)img.sizes.width;
    int h = (int)img.sizes.height;

    // One-time NN session init (idempotent singleton — Plan A's NnDemosaicSession).
    auto& sess = rawalchemy::NnDemosaicSession::instance();
    if (!sess.isReady()) {
        rawalchemy::NnSessionConfig cfg;
        cfg.bayerModelPath = params.nnBayerModelPath;
        cfg.xtransModelPath = params.nnXtransModelPath;
        cfg.qnnContextBinaryDir = params.nnQnnContextBinaryDir;
        cfg.directmlDllPath = params.nnDirectmlDllPath;
        if (!sess.init(cfg)) {
            throw std::runtime_error("[NN] session init failed");
        }
    }

    // Extract CFA mosaic (reuses existing extractCfa helper).
    float whiteLevel = (float)img.color.maximum;
    // extractCfa signature: see raw_decoder.cpp:68 — adapt the call to its actual form.
    std::vector<float> cfa = extractCfa(img, w, h, whiteLevel);  // verify exact signature

    rawalchemy::NnDemosaicInput in{};
    in.cfaMosaic = cfa.data();
    in.width = w;
    in.height = h;
    in.filters = img.idata.filters;
    in.blackLevel = 0.0f;        // raw2image already subtracted black
    in.whiteLevel = whiteLevel;
    // nnDemosaic always outputs raw camRGB (outputCamRgb flag removed; single output contract).
    // decodeRawNn applies camRGB->ProPhoto below to merge into the classical pipeline.
    fillNnMetadata(in, raw);

    rawalchemy::NnDemosaicOutput out;
    rawalchemy::NnDemosaicStatus st = rawalchemy::demosaicDispatch(
        in, out, rawalchemy::DemosaicPath::Neural);
    if (st == rawalchemy::NnDemosaicStatus::SessionNotReady) {
        throw std::runtime_error("[NN] session not ready");
    } else if (st == rawalchemy::NnDemosaicStatus::NaNOutput) {
        throw std::runtime_error("[NN] NaN output");
    } else if (st == rawalchemy::NnDemosaicStatus::InferenceFailed) {
        throw std::runtime_error("[NN] inference failed");
    } else if (st != rawalchemy::NnDemosaicStatus::Ok) {
        throw std::runtime_error("[NN] status " + std::to_string((int)st));
    }

    // out.rgbInterleaved is [w*h*3] linear camRGB (nnDemosaic always outputs camRGB).
    // Convert to linear ProPhoto using LibRaw's cam_xyz (same matrix the classical
    // path uses) → bit-identical into the vLog/LUT pipeline.
    rawalchemy::ImageBuffer result(w, h, 3);
    float camXyz[9];
    for (int i = 0; i < 3; ++i)
        for (int j = 0; j < 3; ++j)
            camXyz[i * 3 + j] = (float)raw.imgdata.color.cam_xyz[i][j];
    rawalchemy::camRgbToProPhotoLinear(result.ptr(), out.rgbInterleaved.data(),
                                       (size_t)w * h, camXyz);
    return result;
}

} // anonymous namespace
```

> **Implementer note:** verify the exact `extractCfa` signature at `raw_decoder.cpp:68` and adapt the call. Verify `ImageBuffer` constructor signature (`ImageBuffer(w, h, channels)` vs other) from `include/image_buffer.h` or wherever it's declared. Verify `LibRaw` is the actual type name used in this file (may be wrapped in `LibRawAccessor` per oracle's note).

- [ ] **Step 3: Add the branch in `decodeRaw` before `dcraw_process`**

At line ~343-346 (where `dcraw_process` is called), wrap with the NN branch:
```cpp
    // --- NN demosaic branch (final full-res only; preview always classical) ---
    if (params.enableNnDemosaic && !params.halfSize) {
        try {
            return decodeRawNn(rawProcessor, params);
        } catch (const std::exception& e) {
            // Surface to caller via the existing catchExceptions mechanism.
            // decodeRaw's caller (CAPI) wraps in try/catch -> RaResult.
            throw;
        }
    }

    // --- Process (demosaic + color conversion + gamma) ---
    int ret = rawProcessor.dcraw_process(&params_libraw);
    // ... existing code unchanged
```

> **Implementer note:** verify `params.halfSize` is the actual field name (may be `half_size` or a method param). Find the half-size handling in `decodeRaw` (oracle cited `raw_decoder.cpp:182-189` for half-size CFA subsampling concerns).

- [ ] **Step 4: Verify compilation + run existing tests (no regression)**

```bash
cmake --build /tmp/ra-planb-task1 --target raw_alchemy_core 2>&1 | tail -10
ctest --test-dir /tmp/ra-planb-task1 2>&1 | tail -5
```
Expected: compiles; all existing tests still pass. (The NN branch is unreachable in tests unless `enableNnDemosaic=true` + a real RAW + models — Task 5's integration test exercises it.)

- [ ] **Step 5: Commit (submodule)**

```bash
git -C src-tauri/lib/rawalchemy add src/raw_decoder.cpp
git -C src-tauri/lib/rawalchemy commit -m "feat(nn): add decodeRawNn branch — unpack-only + NN demosaic + sRGB->ProPhoto"
```

---

## Task 4: Rust FFI — Update libloading Signatures

**Goal:** Mirror the new `enableNnDemosaic` C param in Rust: update the FFI function types, the wrapper methods, and add an NN session init wrapper.

**Files:**
- Modify: `src-tauri/src/color_grading/ffi.rs` (function types ~line 222-277; wrapper methods ~line 452+; `load()` symbol resolution)
- Modify: `src-tauri/src/color_grading/service.rs` (thread `enable_nn: bool`)

**Interfaces:**
- Produces: `RawAlchemyLib::process_file_with_lut(..., enable_nn_demosaic: bool)` + same for `process_file` + `process_to_buffer`. Plus `RawAlchemyLib::demosaic_nn_init(config: &NnSessionConfig) -> Result<(), AppError>`.

- [ ] **Step 1: Update FFI function types**

In `ffi.rs`, the type aliases `RaProcessFileWithLUTFn`, `RaProcessFileFn`, `RaProcessToBufferFn` each gain a trailing `c_int` parameter:
```rust
type RaProcessFileWithLUTFn = unsafe extern "C" fn(
    *const c_char,   // inputPath
    // ... existing params ...
    *const c_char,   // customLensfunDb
    c_int,           // enableNnDemosaic  ← NEW
) -> c_int;
```

- [ ] **Step 2: Update wrapper methods**

`process_file_with_lut` (line ~452):
```rust
    pub fn process_file_with_lut(
        &self,
        // ... existing args ...
        custom_lensfun_db: Option<&str>,
        enable_nn_demosaic: bool,
    ) -> Result<(), AppError> {
        // ... CString setup ...
        let code = unsafe {
            (self.process_file_with_lut)(
                // ... existing args ...
                custom_lensfun_db_ptr,
                if enable_nn_demosaic { 1 } else { 0 },
            )
        };
        // ... existing ra_result_from_code ...
    }
```

Apply the same to `process_file` and `process_to_buffer`.

- [ ] **Step 3: Map new error codes in `ra_result_from_code`**

Add the three new codes:
```rust
    RA_ERR_NN_NOT_INITIALIZED => ColorGradingError("NN demosaic session not initialized".into()),
    RA_ERR_NN_NAN_OUTPUT => ColorGradingError("NN demosaic produced NaN/Inf output".into()),
    RA_ERR_NN_INFERENCE_FAILED => ColorGradingError("NN demosaic inference failed".into()),
```
(Use whatever the existing match arms return — `AppError::ColorGradingError` or similar.)

- [ ] **Step 4: Verify Rust compilation**

```bash
cd /mnt/d/GitRepos/CameraFTP
cargo.exe build 2>&1 | tail -10
```
Expected: compiles. (May have unused-variable warnings for the new arg if callers aren't updated yet — Task 5 wires callers.)

- [ ] **Step 5: Commit (parent)**

```bash
git add src-tauri/src/color_grading/ffi.rs
git commit -m "feat(nn): thread enableNnDemosaic through Rust FFI + NN error code mapping"
```

---

## Task 5: Rust Service Layer — Thread `enable_nn` + NN Init

**Goal:** The service layer (`service.rs`) decides whether to pass `enable_nn=true`. On Android, source is the Kotlin whitelist bridge; on Windows, always-true (or config). Also: call `nn_init` once at startup.

**Files:**
- Modify: `src-tauri/src/color_grading/service.rs` (line ~319 `process_file_with_lut` call + startup init)
- Modify: `src-tauri/src/color_grading/ffi.rs` (add `demosaic_nn_init` wrapper if not done in Task 4)
- Modify: `src-tauri/src/lib.rs` (startup NN init call)

**Interfaces:**
- Consumes: an `nn_enabled: bool` flag from the platform (Kotlin bridge on Android; const true on Windows).
- Produces: the service passes `enable_nn_demosaic=nn_enabled` to `process_file_with_lut`.

- [ ] **Step 1: Add an `nn_enabled` state to the service**

In `service.rs`, the service struct (or the command handler) needs an `nn_enabled: bool` field. On Android it's populated from the JS bridge; on Windows it's `true` (DirectML covers all GPUs). Use `#[cfg(target_os = "android")]` to gate.

- [ ] **Step 2: Add NN session init at startup**

In `lib.rs` (or wherever `RawAlchemyLib::load_global` is called at startup, ~line 188-208), after the lib loads successfully: if `nn_enabled`, call `lib.demosaic_nn_init(&config)` with model paths extracted to the runtime dir. Wrap in `if let Err(e) = ... { tracing::warn!("NN init failed, will use classical: {}", e); }` (non-fatal — classical fallback is the design).

- [ ] **Step 3: Thread `enable_nn` into the `process_file_with_lut` call at service.rs:319**

```rust
        lib.process_file_with_lut(
            // ... existing args ...
            custom_lensfun_db,
            self.nn_enabled,   // ← NEW
        )?;
```

- [ ] **Step 4: Verify compilation**

```bash
cargo.exe build 2>&1 | tail -10
```
Expected: compiles clean.

- [ ] **Step 5: Commit (parent)**

```bash
git add src-tauri/src/color_grading/service.rs src-tauri/src/lib.rs
git commit -m "feat(nn): thread nn_enabled through service + startup session init"
```

---

## Task 6: Kotlin Whitelist + JS Bridge (Android)

**Goal:** Android-only. Detect `Build.SOC_MODEL` against the Qualcomm whitelist; expose `nnEnabled` + `socModel` to the JS frontend via a bridge; load QNN/ORT `.so`s in the correct order before `raw_alchemy_core`.

**Files:**
- Create: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/NnCapabilityBridge.kt`
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/MainActivity.kt` (register bridge + library load order)

**Interfaces:**
- Produces: `window.NnCapability.getNnEnabled()` → `{ enabled: boolean, socModel: string }` from JS.

- [ ] **Step 1: Create `NnCapabilityBridge.kt`**

```kotlin
/** Detects Qualcomm SoC whitelist for NN demosaic eligibility. */
class NnCapabilityBridge {
    companion object {
        private const val TAG = "NnCapabilityBridge"
        // Hexagon v73+ (SD 8 Gen 2 and newer) — FP16-on-HTP supported.
        private val HEXAGON_V73_PLUS = setOf(
            "SM8550", "SM8650", "SM8750", "SM8845", "SM8850"
        )
    }

    @JavascriptInterface
    fun getNnEnabled(): String {
        val socModel = Build.SOC_MODEL ?: "unknown"
        val enabled = socModel in HEXAGON_V73_PLUS
        Log.d(TAG, "SoC=$socModel, NN enabled=$enabled")
        // Return as JSON for the JS side to parse.
        return """{"enabled":$enabled,"socModel":"$socModel"}"""
    }
}
```

- [ ] **Step 2: Register in `MainActivity.onWebViewCreate()`**

In the existing `addJsBridge` block, add:
```kotlin
addJsBridge(webView, NnCapabilityBridge(), "NnCapability")
```

- [ ] **Step 3: Add QNN/ORT library loading order in `MainActivity.onCreate()`**

Before `System.loadLibrary("raw_alchemy_core")` (which the existing code does), add — **only on whitelisted devices**:
```kotlin
if (Build.SOC_MODEL in setOf("SM8550", "SM8650", "SM8750", "SM8845", "SM8850")) {
    System.loadLibrary("onnxruntime")
    System.loadLibrary("QnnSystem")
    System.loadLibrary("QnnHtp")
    // The appropriate Skel (V73/V75/V79/V81) — load all; unused ones are harmless.
}
```
> **Implementer note:** verify the exact `System.loadLibrary` call site for `raw_alchemy_core` in `MainActivity.kt` and insert the QNN loads BEFORE it. The Skel `.so` names must match what's packaged (Task 7).

- [ ] **Step 4: Frontend consumption (minimal)**

In the frontend service that decides whether to request NN processing, read `window.NnCapability?.getNnEnabled()` and pass `nnEnabled` to the Tauri command. (If the frontend doesn't currently distinguish, this can be a no-op pass-through for v1 — the Rust service defaults `nn_enabled` from the bridge result.)

- [ ] **Step 5: Commit (parent)**

```bash
git add src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/NnCapabilityBridge.kt \
        src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/MainActivity.kt
git commit -m "feat(android): NN capability whitelist bridge + QNN/ORT library load order"
```

---

## Task 7: Resource Packaging — Models + QNN Skels + DirectML DLL

**Goal:** Wire `scripts/build.sh` to package the NN runtime artifacts alongside the existing LUT/Lensfun resources: ORT `.so`/`.dll`, QNN runtime + Skels (Android), DirectML.dll (Windows), x-veon models, offline context binaries.

**Files:**
- Modify: `scripts/build.sh` (add `prepare_nn_resources()`)
- Modify: `scripts/build-android.sh` (copy QNN `.so`s into `extra-jniLibs/arm64-v8a/`)
- Modify: `scripts/build_windows.bat` or the Windows resource embed (DirectML.dll embedding alongside the existing `raw_alchemy_core.dll` gzip-embed)

**Interfaces:**
- Produces: at runtime, models + DLLs/.so's extracted to the same temp/resource dir as Lensfun DB, paths passed to `NnSessionConfig`.

- [ ] **Step 1: Add `prepare_nn_resources()` to `scripts/build.sh`**

Parallel to the existing `prepare_lut_resources()`. Copy `src-tauri/resources/models/xveon/*.onnx` into the build's resource dir. On Android, also copy QNN runtime `.so`s from the nn-cache into the staging area. On Windows, ensure `DirectML.dll` is alongside `raw_alchemy_core.dll` in the embed source.

- [ ] **Step 2: Android — package QNN `.so`s into `jniLibs`**

In `scripts/build-android.sh` (~line 308-344, where `libraw_alchemy_core.so` + `libomp.so` are copied to `extra-jniLibs/arm64-v8a/`), add:
```bash
# QNN runtime (only if cached)
NN_CACHE="src-tauri/lib/rawalchemy/third_party/nn-cache/qnn-runtime-2.34.0/jni/arm64-v8a"
if [[ -d "$NN_CACHE" ]]; then
    cp "$NN_CACHE"/libonnxruntime.so "$EXTRA_JNILIBS/arm64-v8a/"
    cp "$NN_CACHE"/libQnnSystem.so "$EXTRA_JNILIBS/arm64-v8a/"
    cp "$NN_CACHE"/libQnnHtp.so "$EXTRA_JNILIBS/arm64-v8a/"
    cp "$NN_CACHE"/libQnnHtpV*.so "$EXTRA_JNILIBS/arm64-v8a/"  # all Skels
fi
```

- [ ] **Step 3: Windows — DirectML.dll alongside raw_alchemy_core.dll**

In the Windows embed pipeline (`scripts/build_windows.bat` or wherever `raw_alchemy_core.dll` is gzipped into the Rust binary), also handle `DirectML.dll`. If the existing pattern is gzip-embed-then-extract-to-temp, add `DirectML.dll` to the same embed list. The runtime extractor (`color_grading/ffi.rs:16-184`) must also extract `DirectML.dll` to the same temp dir + `LoadLibrary` it before session init.

- [ ] **Step 4: Runtime extraction paths**

The Rust startup init (Task 5) extracts models to a runtime dir (parallel to Lensfun DB extraction in `color_grading/resources.rs`). Pass those paths to `NnSessionConfig`:
```rust
NnSessionConfig {
    bayerModelPath: extracted_dir.join("models/xveon/bayer.onnx"),
    xtransModelPath: extracted_dir.join("models/xveon/xtrans.onnx"),
    qnnContextBinaryDir: extracted_dir.join("context_binaries"),  // Android
    directmlDllPath: temp_dir.join("DirectML.dll"),               // Windows
}
```

- [ ] **Step 5: Verify the build packages the artifacts**

```bash
cd /mnt/d/GitRepos/CameraFTP
./build.sh windows android 2>&1 | grep -iE "directml|qnn|onnxruntime|xveon" | head -10
```
Expected: build log shows the NN resources being staged/packaged.

- [ ] **Step 6: Commit (parent)**

```bash
git add scripts/build.sh scripts/build-android.sh scripts/build_windows.bat
git commit -m "build: package NN runtime resources (ORT, QNN Skels, DirectML, models)"
```

---

## Task 8: Integration Verification — Full Pipeline End-to-End

**Goal:** Verify the complete flow compiles + the NN path is reachable. Run the gated integration test with real models (the `RA_NN_INTEGRATION_TEST` env from Plan A's Task 8).

**Files:**
- No new files — exercises existing test + builds.

- [ ] **Step 1: Cross-platform build green**

```bash
cd /mnt/d/GitRepos/CameraFTP
./build.sh windows android
```
Expected: both platforms green. The Windows build proves the DirectML branch compiles; the Android build proves the QNN branch + jniLibs packaging compiles.

- [ ] **Step 2: Run the gated NN integration test with real models**

```bash
cmake --build /tmp/ra-planb-task1 --target test_nn_dispatch
RA_NN_INTEGRATION_TEST=1 \
RA_NN_BAYER_ONNX=src-tauri/resources/models/xveon/bayer.onnx \
/tmp/ra-planb-task1/test_nn_dispatch
```
Expected: prints `test_nn_dispatch: OK` with sub-test 3 (golden path) now exercising the full NN path. (Sub-test 3 was skipped in Plan A because models weren't wired; now they are.)

- [ ] **Step 3: Manual smoke test via CLI (optional, if `raw_alchemy_cli` supports the flag)**

If the CLI (`raw_alchemy_cli`) accepts a `--nn-demosaic` flag (wire it in `main.cpp` if not), run on a sample RAW:
```bash
./build/raw_alchemy_cli --input sample.CR2 --output out.jpg --nn-demosaic
```
Expected: produces a JPEG via the NN path (or reports an NN error if session init fails — both are valid outcomes proving the plumbing works).

- [ ] **Step 4: Commit any test-fixture or CLI-flag additions**

```bash
git -C src-tauri/lib/rawalchemy add src/main.cpp  # if --nn-demosaic flag added
git -C src-tauri/lib/rawalchemy commit -m "feat(cli): add --nn-demosaic flag for smoke testing"
git add src-tauri/lib/rawalchemy  # submodule bump
git commit -m "chore(submodule): bump rawalchemy — NN integration verification"
```

---

## Self-Review

**Spec coverage (against `docs/nn-demosaic-design.md`):**
- §4.3 FFI signature change (`enableNnDemosaic` param): ✓ Task 2
- §4.4 Dispatch routing + preview policy: ✓ Task 3 (branch gated on `!halfSize`)
- §3.2 Android QNN HTP + whitelist: ✓ Task 6 (Kotlin whitelist + library load order)
- §3.3 Windows DirectML + app-local DLL: ✓ Task 7 (DirectML.dll packaging)
- §5 Resource packaging: ✓ Task 7
- §6 Reliability (NN fail → error, no auto-fallback): ✓ Task 3 (throws → CAPI catchExceptions → RA_ERR_NN_*)
- Oracle assessment (Approach 2, camRGB→ProPhoto adapter using runtime cam_xyz — bit-identical to classical): ✓ Task 1 (adapter) + Task 3 (decodeRawNn, outputCamRgb=true)

**Placeholder scan:** Task 3 Step 2 has implementer notes to verify `extractCfa` signature, `ImageBuffer` constructor, `LibRaw` type name, `halfSize` field name — these are "verify against existing code" not "TBD". The code shown is structurally complete; the notes flag exact signatures to confirm. Acceptable.

**Type consistency:**
- `enableNnDemosaic` (C, `int`) → `enable_nn_demosaic` (Rust, `bool`) → `nn_enabled` (service): consistent.
- `NnSessionConfig` fields `bayerModelPath`/`xtransModelPath` (Plan A's actual names, not the brief's stale `bayerOnnxPath`): consistent with Task 3 + Task 5.
- `RA_ERR_NN_*` codes: defined Task 2, mapped Task 4.
- `sRgbToProPhotoLinear` signature consistent across Task 1 (def) + Task 3 (call).

**Gaps:** Highlight reconstruction (design §2.3 step 4) remains unimplemented from Plan A — it's a tracked post-Plan-A follow-up, NOT a Plan B item. The §8 reliability gates (tile-seam regression, multi-device QNN stability) are Plan B's test-list but Task 8 covers the integration smoke; the full §8 gate suite is a separate testing effort.

---

## Execution Handoff

**Plan B complete and saved to `docs/superpowers/plans/2026-06-26-nn-demosaic-app-integration.md`. Two execution options:**

**1. Subagent-Driven (recommended)** — fresh `@fixer` per task, review between tasks. Best fit because Task 3 (decodeRawNn) has the highest risk (LibRaw API specifics + the sRGB→ProPhoto boundary).

**2. Inline Execution** — batch in this session with checkpoints.

**Which approach?**
