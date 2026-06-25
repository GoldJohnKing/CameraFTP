# NN Demosaic (x-veon) C++ Core Implementation Plan — Plan A

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Integrate the x-veon neural-network demosaic model into RawAlchemyCpp as a new `demosaic_nn_xveon.cpp` module, selectable at runtime via a dispatch flag, with full preprocessing/postprocessing pipeline and unit-tested primitives.

**Architecture:** A new C++ module inside `src-tauri/lib/rawalchemy/src/` runs ONNX inference (via ONNX Runtime — QNN HTP EP on Android, DirectML EP on Windows) on 288×288 tiles of the CFA mosaic. The existing empty `demosaic_dispatch.cpp` seam becomes the runtime router. The existing `raProcessFileWithLUT` pipeline is the integration point (Plan B adds the `enableNnDemosaic` parameter; Plan A exposes an internal C++ entry point `nnDemosaic()` that Plan B's FFI change will call). All preprocessing (CFA normalization, phase alignment, mask construction, tiling) and postprocessing (trapezoidal blend, camRGB→sRGB matrix) are pure functions with unit tests.

**Tech Stack:** C++17, CMake (ExternalProject for ORT/DirectML, Maven fetch for QNN runtime), ONNX Runtime 1.24.x (Windows DirectML) / onnxruntime-android + onnxruntime-qnn (Android), OpenMP (existing), x-veon ONNX models (opset 17, fp16).

**Spec reference:** [`docs/nn-demosaic-design.md`](../../nn-demosaic-design.md) — all algorithm formulas (preprocessing §2.3, postprocessing §2.4, tile params §2.5) are defined there verbatim. This plan references sections like "design §2.3 step 3" rather than duplicating formulas.

## Global Constraints

- **Build commands:** NEVER use `cargo` or `npm` directly. Use `./build.sh windows android` for full builds. For RawAlchemyCpp-only iteration during this plan, use the standalone CMake build documented in Task 1 Step 2.
- **`cargo.exe` not `cargo`:** Cross-platform project; you are in WSL2, call `cargo.exe` for Windows artifacts.
- **No LSP tools:** `lsp_*` tools hang. Use `grep`/`glob`/compile errors instead.
- **License headers:** Every new `.cpp`/`.h` file starts with the AGPL-3.0-or-later SPDX header (see existing files for format).
- **Test convention:** Tests are standalone `main()` programs using `<cassert>`, no framework. Live in `src-tauri/lib/rawalchemy/Test/cpp/`. Include headers via `../../include/...`. Enabled via CMake `-DBUILD_TESTS=ON`.
- **CFA terminology:** `filters` field from LibRaw encodes CFA pattern. `0x94949494` = RGGB Bayer; `9` = X-Trans. Existing helpers: `bayerColor(y,x,filters)`, `isXtrans(filters)`, `xtransColor(y,x,pattern)` in [`include/cfa_lookup.h`](../../../src-tauri/lib/rawalchemy/include/cfa_lookup.h).
- **No `unwrap()`/`expect()`** in any new code; use `Result`-style or error codes.
- **Fast-math isolation:** `nn_nan_guard.cpp` MUST compile with `-ffast-math` DISABLED (else `isnan`/`isinf` get optimized away). All other new files inherit the core's `-ffast-math`.
- **Commit after every task** (not mid-task). Conventional Commits style: `feat:`, `test:`, `build:`, `chore:`.

---

## File Structure

### New files

| Path | Responsibility |
|---|---|
| `src-tauri/lib/rawalchemy/include/demosaic_nn_xveon.h` | Public header: `nnDemosaic()` signature + `NnDemosaicConfig` struct + `NnDemosaicResult` enum |
| `src-tauri/lib/rawalchemy/src/demosaic_nn_xveon.cpp` | NN inference core: session lifecycle, tile loop, OpenMP parallel tiles, preprocessing dispatch, postprocessing blend |
| `src-tauri/lib/rawalchemy/src/nn_preprocess.cpp` | Pure preprocessing functions: normalize, detectPhase, makeCanonicalMasks, packTileInput, mirrorPadToAlignment |
| `src-tauri/lib/rawalchemy/include/nn_preprocess.h` | Header for above |
| `src-tauri/lib/rawalchemy/src/nn_postprocess.cpp` | Pure postprocessing functions: trapezoidalBlend weights, applyColorMatrix, cropToHwc |
| `src-tauri/lib/rawalchemy/include/nn_postprocess.h` | Header for above |
| `src-tauri/lib/rawalchemy/src/nn_nan_guard.cpp` | `nnOutputHasNaNInf()` — compiled WITHOUT `-ffast-math` |
| `src-tauri/lib/rawalchemy/include/nn_nan_guard.h` | Header for above |
| `src-tauri/lib/rawalchemy/src/nn_session.cpp` | ORT session singleton: init, EP registration (QNN HTP / DirectML), model loading |
| `src-tauri/lib/rawalchemy/include/nn_session.h` | Header for above |
| `src-tauri/lib/rawalchemy/Test/cpp/test_nn_preprocess.cpp` | Unit tests for preprocessing |
| `src-tauri/lib/rawalchemy/Test/cpp/test_nn_postprocess.cpp` | Unit tests for postprocessing |
| `src-tauri/lib/rawalchemy/Test/cpp/test_nn_nan_guard.cpp` | Unit tests for NaN guard |
| `src-tauri/lib/rawalchemy/Test/cpp/test_nn_dispatch.cpp` | Integration test: demosaic a synthetic CFA, assert no NaN, assert output shape |

### Modified files

| Path | Change |
|---|---|
| `src-tauri/lib/rawalchemy/CMakeLists.txt` | Add ExternalProject for ORT (Windows) + DirectML; add QNN runtime fetch (Android); add new source files to `raw_alchemy_core`; add `nn_nan_guard.cpp` with fast-math disabled; add `RA_ENABLE_NN_DEMOSAIC` option |
| `src-tauri/lib/rawalchemy/src/demosaic_dispatch.cpp` | Implement runtime router: `demosaicDispatch(ctx, enable_nn)` |
| `src-tauri/lib/rawalchemy/include/demosaic_dispatch.h` | New header declaring `demosaicDispatch()` |

### Resource files (committed, not generated)

| Path | Purpose |
|---|---|
| `src-tauri/lib/rawalchemy/Test/data/bayer_test_cfa.bin` | Synthetic 64×64 Bayer CFA fixture for unit/integration tests |
| `src-tauri/lib/rawalchemy/Test/data/bayer_test_expected.rgb` | Expected demosaic output (golden image) for integration test |
| `src-tauri/resources/models/xveon/bayer.onnx` | x-veon Bayer model (~4MB fp16) |
| `src-tauri/resources/models/xveon/xtrans.onnx` | x-veon X-Trans model (~15.5MB fp16) |

---

## Task 1: Build System — Vendor ONNX Runtime & DirectML

**Goal:** Make `raw_alchemy_core` link against ORT on both platforms. No NN code yet — just get the dependency building.

**Files:**
- Modify: `src-tauri/lib/rawalchemy/CMakeLists.txt`
- Create: `scripts/fetch-nn-deps.sh` (download helper, called by CMake or build.sh)

**Interfaces:**
- Produces: CMake targets `onnxruntime::dynamic` (alias) and `directml::lib` (Windows only), and `RA_ENABLE_NN_DEMOSAIC` cache option (default ON).

- [ ] **Step 1: Add the RA_ENABLE_NN_DEMOSAIC option and ORT detection scaffold**

Open `src-tauri/lib/rawalchemy/CMakeLists.txt`. After line 258 (`option(ENABLE_LENS_CORRECTION ...)`), add:

```cmake
# ---- Neural Network Demosaic (x-veon via ONNX Runtime) ----
option(RA_ENABLE_NN_DEMOSAIC "Enable x-veon NN demosaic via ONNX Runtime" ON)
set(RA_ORT_ROOT "" CACHE PATH "Path to pre-extracted ONNX Runtime SDK root (contains lib/ and include/). If empty, fetched automatically.")
set(RA_DIRECTML_DLL "" CACHE FILEPATH "Path to DirectML.dll (Windows only). If empty, fetched automatically.")
set(RA_QNN_RUNTIME_DIR "" CACHE PATH "Path to dir containing libQnnHtp.so + Skels (Android only). If empty, fetched from Maven.")
```

- [ ] **Step 2: Create the fetch helper script**

Create `scripts/fetch-nn-deps.sh`:

```bash
#!/bin/bash
# Fetches ONNX Runtime + DirectML (Windows) or ORT-android + QNN runtime (Android)
# into a vendored cache dir. Idempotent. Called by build.sh or manually.
set -euo pipefail

CACHE_DIR="${1:-third_party/nn-cache}"
mkdir -p "$CACHE_DIR"

# Pin versions — update here, run script, commit the cache (or gitignore + refetch).
ORT_VERSION="1.24.1"          # latest stable at plan time; verify at onnxruntime.ai
QNN_RUNTIME_VERSION="2.34.0"  # com.qualcomm.qti:qnn-runtime Maven
DIRECTML_VERSION="1.15.4"     # Microsoft.AI.DirectML NuGet

# --- Windows ORT + DirectML ---
if [[ ! -f "$CACHE_DIR/onnxruntime-win-x64-$ORT_VERSION.zip" ]]; then
    curl -fL "https://github.com/microsoft/onnxruntime/releases/download/v$ORT_VERSION/onnxruntime-win-x64-$ORT_VERSION.zip" \
        -o "$CACHE_DIR/onnxruntime-win-x64-$ORT_VERSION.zip"
    unzip -qo -d "$CACHE_DIR" "$CACHE_DIR/onnxruntime-win-x64-$ORT_VERSION.zip"
fi
if [[ ! -f "$CACHE_DIR/DirectML.dll" ]]; then
    # Pull DirectML.dll from the Microsoft.AI.DirectML NuGet package
    curl -fL "https://www.nuget.org/api/v2/package/Microsoft.AI.DirectML/$DIRECTML_VERSION" \
        -o "$CACHE_DIR/directml.nupkg.zip"
    unzip -qo -j "$CACHE_DIR/directml.nupkg.zip" "bin/x64-win/DirectML.dll" -d "$CACHE_DIR"
fi

# --- Android ORT (arm64) + QNN runtime ---
if [[ ! -d "$CACHE_DIR/onnxruntime-android-$ORT_VERSION" ]]; then
    curl -fL "https://repo1.maven.org/maven2/com/microsoft/onnxruntime/onnxruntime-android/$ORT_VERSION/onnxruntime-android-$ORT_VERSION.aar" \
        -o "$CACHE_DIR/ort-android.aar"
    mkdir -p "$CACHE_DIR/onnxruntime-android-$ORT_VERSION"
    unzip -qo "$CACHE_DIR/ort-android.aar" -d "$CACHE_DIR/onnxruntime-android-$ORT_VERSION"
fi
if [[ ! -d "$CACHE_DIR/qnn-runtime-$QNN_RUNTIME_VERSION" ]]; then
    curl -fL "https://repo1.maven.org/maven2/com/qualcomm/qti/qnn-runtime/$QNN_RUNTIME_VERSION/qnn-runtime-$QNN_RUNTIME_VERSION.aar" \
        -o "$CACHE_DIR/qnn-runtime.aar"
    mkdir -p "$CACHE_DIR/qnn-runtime-$QNN_RUNTIME_VERSION"
    unzip -qo "$CACHE_DIR/qnn-runtime.aar" -d "$CACHE_DIR/qnn-runtime-$QNN_RUNTIME_VERSION"
fi

echo "NN deps cached in $CACHE_DIR"
```

Make executable: `chmod +x scripts/fetch-nn-deps.sh`.

- [ ] **Step 3: Wire ORT into CMakeLists (Windows path)**

In `CMakeLists.txt`, after the new option block from Step 1, add:

```cmake
if(RA_ENABLE_NN_DEMOSAIC)
    # Auto-fetch if root not provided
    if(NOT RA_ORT_ROOT)
        set(RA_NN_CACHE "${CMAKE_SOURCE_DIR}/third_party/nn-cache")
        if(WIN32)
            set(RA_ORT_ROOT "${RA_NN_CACHE}/onnxruntime-win-x64-${ORT_VERSION}" CACHE PATH "" FORCE)
        elseif(ANDROID)
            set(RA_ORT_ROOT "${RA_NN_CACHE}/onnxruntime-android-${ORT_VERSION}" CACHE PATH "" FORCE)
        endif()
    endif()

    if(WIN32)
        # ORT Windows: headers + import lib + DLL
        set(ORT_INCLUDE_DIR "${RA_ORT_ROOT}/include")
        set(ORT_LIB_DIR "${RA_ORT_ROOT}/lib")
        add_library(onnxruntime::dynamic SHARED IMPORTED)
        set_target_properties(onnxruntime::dynamic PROPERTIES
            IMPORTED_LOCATION "${RA_ORT_ROOT}/lib/onnxruntime.dll"
            IMPORTED_IMPLIB "${ORT_LIB_DIR}/onnxruntime.lib"
            INTERFACE_INCLUDE_DIRECTORIES "${ORT_INCLUDE_DIR}"
        )
        # DirectML.dll (app-local, loaded manually via LoadLibrary)
        set(DIRECTML_DLL_PATH "${RA_NN_CACHE}/DirectML.dll" CACHE FILEPATH "" FORCE)
        message(STATUS "NN Demosaic: ORT=${RA_ORT_ROOT}, DirectML=${DIRECTML_DLL_PATH}")

    elseif(ANDROID)
        # ORT Android: headers + .so inside the extracted AAR
        set(ORT_INCLUDE_DIR "${RA_ORT_ROOT}/headers")
        set(ORT_LIB_DIR "${RA_ORT_ROOT}/jni/arm64-v8a")
        add_library(onnxruntime::dynamic SHARED IMPORTED)
        set_target_properties(onnxruntime::dynamic PROPERTIES
            IMPORTED_LOCATION "${ORT_LIB_DIR}/libonnxruntime.so"
            INTERFACE_INCLUDE_DIRECTORIES "${ORT_INCLUDE_DIR}"
        )
        # QNN runtime .so files (libQnnHtp.so, Skels, etc.) for packaging
        set(QNN_RUNTIME_DIR "${RA_NN_CACHE}/qnn-runtime-${QNN_RUNTIME_VERSION}/jni/arm64-v8a" CACHE PATH "" FORCE)
        message(STATUS "NN Demosaic: ORT=${RA_ORT_ROOT}, QNN=${QNN_RUNTIME_DIR}")
    endif()
endif()
```

- [ ] **Step 4: Verify build still compiles (NN code not yet used)**

Run a Windows config-only test to confirm CMake accepts the new options:

```bash
cd /mnt/d/GitRepos/CameraFTP
./scripts/fetch-nn-deps.sh   # populates third_party/nn-cache
# Configure only (do not build) to validate CMake parsing:
cmake -S src-tauri/lib/rawalchemy -B /tmp/ra-cfg-test -DBUILD_CLI=OFF -DBUILD_SHARED=ON -DBUILD_CAPI=ON -DRA_ENABLE_NN_DEMOSAIC=ON
```
Expected: CMake configures successfully, prints `NN Demosaic: ORT=..., DirectML=...`.

- [ ] **Step 5: Commit**

```bash
git add scripts/fetch-nn-deps.sh src-tauri/lib/rawalchemy/CMakeLists.txt
git commit -m "build: vendor ONNX Runtime + DirectML + QNN runtime for NN demosaic"
```

---

## Task 2: NaN Guard Module (TDD)

**Goal:** The simplest module first. `nn_nan_guard.cpp` scans a float buffer for NaN/Inf. Must compile WITHOUT `-ffast-math`.

**Files:**
- Create: `src-tauri/lib/rawalchemy/include/nn_nan_guard.h`
- Create: `src-tauri/lib/rawalchemy/src/nn_nan_guard.cpp`
- Create: `src-tauri/lib/rawalchemy/Test/cpp/test_nn_nan_guard.cpp`
- Modify: `src-tauri/lib/rawalchemy/CMakeLists.txt` (add source file + fast-math override for this TU)

**Interfaces:**
- Produces: `bool nnOutputHasNaNInf(const float* data, size_t count)` — returns true if any element is NaN or Inf.

- [ ] **Step 1: Write the failing test**

Create `src-tauri/lib/rawalchemy/Test/cpp/test_nn_nan_guard.cpp`:

```cpp
// SPDX-License-Identifier: AGPL-3.0-or-later
#include "../../include/nn_nan_guard.h"

#include <cassert>
#include <cmath>
#include <iostream>

int main() {
    // Clean buffer -> false
    {
        float data[] = {0.0f, 1.0f, -1.0f, 0.5f, 1000.0f};
        assert(nnOutputHasNaNInf(data, 5) == false);
    }
    // Contains NaN -> true
    {
        float data[] = {0.0f, NAN, 1.0f};
        assert(nnOutputHasNaNInf(data, 3) == true);
    }
    // Contains +Inf -> true
    {
        float data[] = {1.0f, INFINITY, 2.0f};
        assert(nnOutputHasNaNInf(data, 3) == true);
    }
    // Contains -Inf -> true
    {
        float data[] = {-INFINITY, 0.0f};
        assert(nnOutputHasNaNInf(data, 2) == true);
    }
    // Zero count -> false
    {
        assert(nnOutputHasNaNInf(nullptr, 0) == false);
    }
    std::cout << "test_nn_nan_guard: OK\n";
    return 0;
}
```

- [ ] **Step 2: Run the test to verify it fails**

```bash
cd /mnt/d/GitRepos/CameraFTP
cmake -S src-tauri/lib/rawalchemy -B /tmp/ra-build-test -DBUILD_TESTS=ON -DBUILD_CLI=OFF -DBUILD_SHARED=ON -DBUILD_CAPI=ON
cmake --build /tmp/ra-build-test --target test_nn_nan_guard 2>&1 | tail -20
```
Expected: FAIL — `nn_nan_guard.h` not found / `nnOutputHasNaNInf` undefined.

- [ ] **Step 3: Create the header**

Create `src-tauri/lib/rawalchemy/include/nn_nan_guard.h`:

```cpp
// SPDX-License-Identifier: AGPL-3.0-or-later
// NaN/Inf output guard for NN demosaic inference.
// IMPORTANT: the .cpp implementation MUST be compiled WITHOUT -ffast-math,
// otherwise the optimizer will delete isnan()/isinf() checks.
#pragma once
#include <cstddef>

namespace rawalchemy {

/** Returns true if any element of `data` (length `count`) is NaN or +/- Inf.
 *  Safe to call with count == 0 (returns false). `data` may be nullptr if count == 0. */
bool nnOutputHasNaNInf(const float* data, size_t count);

} // namespace rawalchemy
```

- [ ] **Step 4: Create the implementation**

Create `src-tauri/lib/rawalchemy/src/nn_nan_guard.cpp`:

```cpp
// SPDX-License-Identifier: AGPL-3.0-or-later
// DO NOT compile this file with -ffast-math (CMakeLists enforces this).
#include "nn_nan_guard.h"
#include <cmath>

namespace rawalchemy {

bool nnOutputHasNaNInf(const float* data, size_t count) {
    for (size_t i = 0; i < count; ++i) {
        if (std::isnan(data[i]) || std::isinf(data[i])) {
            return true;
        }
    }
    return false;
}

} // namespace rawalchemy
```

- [ ] **Step 5: Register in CMakeLists with fast-math DISABLED for this TU**

In `CMakeLists.txt`, find the `raw_alchemy_core` source list (search for the `set(RA_CORE_SOURCES` or the `add_library(raw_alchemy_core` block). Add `src/nn_nan_guard.cpp` to the sources. Then immediately after the `target_compile_options` for fast-math (around line 578–587 where `-ffast-math` is applied), add a per-file override:

```cmake
# nn_nan_guard.cpp MUST NOT use -ffast-math (would delete isnan/isinf guards)
if(CMAKE_CXX_COMPILER_ID MATCHES "GNU|Clang")
    set_source_files_properties(src/nn_nan_guard.cpp PROPERTIES
        COMPILE_OPTIONS "-fno-fast-math"
    )
elseif(MSVC)
    set_source_files_properties(src/nn_nan_guard.cpp PROPERTIES
        COMPILE_OPTIONS "/fp:strict"
    )
endif()
```

- [ ] **Step 6: Run the test to verify it passes**

```bash
cmake --build /tmp/ra-build-test --target test_nn_nan_guard
/tmp/ra-build-test/test_nn_nan_guard
```
Expected: prints `test_nn_nan_guard: OK`, exit 0.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/lib/rawalchemy/include/nn_nan_guard.h \
        src-tauri/lib/rawalchemy/src/nn_nan_guard.cpp \
        src-tauri/lib/rawalchemy/Test/cpp/test_nn_nan_guard.cpp \
        src-tauri/lib/rawalchemy/CMakeLists.txt
git commit -m "feat(nn): add NaN/Inf output guard with fast-math-disabled compilation"
```

---

## Task 3: Preprocessing Primitives — Normalization & Phase Detection (TDD)

**Goal:** Pure functions for CFA normalization and CFA phase detection. These are the first two preprocessing steps (design §2.3 steps 2–3).

**Files:**
- Create: `src-tauri/lib/rawalchemy/include/nn_preprocess.h`
- Create: `src-tauri/lib/rawalchemy/src/nn_preprocess.cpp`
- Create: `src-tauri/lib/rawalchemy/Test/cpp/test_nn_preprocess.cpp`
- Modify: `src-tauri/lib/rawalchemy/CMakeLists.txt`

**Interfaces:**
- Produces:
  - `struct CfaPhase { int dy; int dx; int period; bool isXtrans; }` — the detected phase offset to canonical RGGB or X-Trans.
  - `CfaPhase detectCfaPhase(unsigned filters)` — returns phase for Bayer; for X-Trans returns `{0,0,6,true}` (X-Trans is canonically aligned by LibRaw).
  - `void normalizeCfaInPlace(float* cfa, size_t count, float blackLevel, float whiteLevel)` — design §2.3 step 3: `(raw - black) / (white - black)`.

- [ ] **Step 1: Write failing tests**

Create `src-tauri/lib/rawalchemy/Test/cpp/test_nn_preprocess.cpp`:

```cpp
// SPDX-License-Identifier: AGPL-3.0-or-later
#include "../../include/nn_preprocess.h"

#include <cassert>
#include <cmath>
#include <iostream>

int main() {
    using namespace rawalchemy;

    // --- normalizeCfaInPlace ---
    {
        float cfa[] = {100.0f, 200.0f, 300.0f, 400.0f};
        normalizeCfaInPlace(cfa, 4, /*black=*/100.0f, /*white=*/300.0f);
        // (raw-100)/(300-100) = (raw-100)/200
        assert(std::fabs(cfa[0] - 0.0f) < 1e-6f);
        assert(std::fabs(cfa[1] - 0.5f) < 1e-6f);
        assert(std::fabs(cfa[2] - 1.0f) < 1e-6f);
        assert(std::fabs(cfa[3] - 1.5f) < 1e-6f);  // HDR pass-through above 1.0
    }

    // --- detectCfaPhase: RGGB ---
    {
        CfaPhase p = detectCfaPhase(0x94949494u);
        assert(p.isXtrans == false);
        assert(p.period == 2);
        assert(p.dy == 0 && p.dx == 0);  // RGGB already canonical
    }

    // --- detectCfaPhase: BGGR (phase-shifted RGGB) ---
    {
        // BGGR = 0x16161616; R is at (1,1), so canonical origin offset is (1,1)
        CfaPhase p = detectCfaPhase(0x16161616u);
        assert(p.isXtrans == false);
        assert(p.period == 2);
        assert(p.dy == 1 && p.dx == 1);
    }

    // --- detectCfaPhase: X-Trans ---
    {
        CfaPhase p = detectCfaPhase(9);  // X-Trans marker
        assert(p.isXtrans == true);
        assert(p.period == 6);
    }

    std::cout << "test_nn_preprocess: OK\n";
    return 0;
}
```

- [ ] **Step 2: Run to verify failure**

```bash
cmake --build /tmp/ra-build-test --target test_nn_preprocess 2>&1 | tail -5
```
Expected: FAIL — header not found.

- [ ] **Step 3: Create the header**

Create `src-tauri/lib/rawalchemy/include/nn_preprocess.h`:

```cpp
// SPDX-License-Identifier: AGPL-3.0-or-later
// Pure preprocessing primitives for x-veon NN demosaic.
// Algorithms: see docs/nn-demosaic-design.md §2.3.
#pragma once
#include <cstddef>
#include "cfa_lookup.h"  // for unsigned filters helpers

namespace rawalchemy {

/** Detected CFA phase relative to canonical RGGB (Bayer) or canonical X-Trans 6x6.
 *  `dy`,`dx` is the top-left offset to mirror-pad so the image origin aligns
 *  to the canonical pattern (R at (0,0) for Bayer). */
struct CfaPhase {
    int dy = 0;
    int dx = 0;
    int period = 2;       // 2 for Bayer, 6 for X-Trans
    bool isXtrans = false;
};

/** Detect the CFA family and phase. For Bayer orientations other than RGGB,
 *  returns the offset needed to align to RGGB origin. For X-Trans returns
 *  period=6, dy=dx=0 (LibRaw delivers canonically-aligned X-Trans). */
CfaPhase detectCfaPhase(unsigned filters);

/** In-place CFA normalization: out = (raw - black) / (white - black).
 *  Single black/white level for all sites (per x-veon training, NOT per-site).
 *  Values may exceed 1.0 for HDR highlights (no upper clamp). */
void normalizeCfaInPlace(float* cfa, size_t count, float blackLevel, float whiteLevel);

} // namespace rawalchemy
```

- [ ] **Step 4: Create the implementation**

Create `src-tauri/lib/rawalchemy/src/nn_preprocess.cpp`:

```cpp
// SPDX-License-Identifier: AGPL-3.0-or-later
#include "nn_preprocess.h"

namespace rawalchemy {

CfaPhase detectCfaPhase(unsigned filters) {
    CfaPhase p;
    if (isXtrans(filters)) {
        p.isXtrans = true;
        p.period = 6;
        p.dy = 0;
        p.dx = 0;
        return p;
    }
    // Bayer: find which corner holds R. Canonical RGGB has R at (0,0).
    // bayerColor returns 0 for R, 1 for G in row 0, 2 for B, 3 for G in row 1.
    p.period = 2;
    // Check the 2x2 origin colors
    int c00 = bayerColor(0, 0, filters);  // expected 0 (R) for RGGB
    if (c00 == 0) { p.dy = 0; p.dx = 0; }        // RGGB
    else if (bayerColor(0, 1, filters) == 0) { p.dy = 0; p.dx = 1; }  // GRBG
    else if (bayerColor(1, 0, filters) == 0) { p.dy = 1; p.dx = 0; }  // GBRG
    else { p.dy = 1; p.dx = 1; }                                        // BGGR
    return p;
}

void normalizeCfaInPlace(float* cfa, size_t count, float blackLevel, float whiteLevel) {
    const float range = whiteLevel - blackLevel;
    if (range <= 0.0f) {
        // Degenerate; zero the buffer to avoid div-by-zero.
        for (size_t i = 0; i < count; ++i) cfa[i] = 0.0f;
        return;
    }
    const float invRange = 1.0f / range;
    for (size_t i = 0; i < count; ++i) {
        cfa[i] = (cfa[i] - blackLevel) * invRange;
    }
}

} // namespace rawalchemy
```

- [ ] **Step 5: Add `src/nn_preprocess.cpp` to `raw_alchemy_core` sources in CMakeLists** (same pattern as Task 2 Step 5, but no fast-math override — this file inherits core fast-math).

- [ ] **Step 6: Run the test to verify it passes**

```bash
cmake --build /tmp/ra-build-test --target test_nn_preprocess
/tmp/ra-build-test/test_nn_preprocess
```
Expected: `test_nn_preprocess: OK`.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/lib/rawalchemy/include/nn_preprocess.h \
        src-tauri/lib/rawalchemy/src/nn_preprocess.cpp \
        src-tauri/lib/rawalchemy/Test/cpp/test_nn_preprocess.cpp \
        src-tauri/lib/rawalchemy/CMakeLists.txt
git commit -m "feat(nn): add CFA normalization and phase detection primitives"
```

---

## Task 4: Preprocessing — Canonical Masks & Tile Packing (TDD)

**Goal:** Mask construction (design §2.3 step 8) and 4-channel tile packing (design §2.3 step 8). These produce the `[1,4,288,288]` input tensor.

**Files:**
- Modify: `src-tauri/lib/rawalchemy/include/nn_preprocess.h` (add declarations)
- Modify: `src-tauri/lib/rawalchemy/src/nn_preprocess.cpp` (add implementations)
- Modify: `src-tauri/lib/rawalchemy/Test/cpp/test_nn_preprocess.cpp` (add tests)

**Interfaces:**
- Produces:
  - `static constexpr int NN_PATCH_SIZE = 288;`
  - `static constexpr int NN_OVERLAP = 48;`
  - `void makeCanonicalMasks(float* outMasksR, float* outMasksG, float* outMasksB, const CfaPhase& phase)` — fills three 288×288 planes with one-hot masks of the canonical pattern.
  - `void packTileInput(float* outTile4ch /* [4*288*288] */, const float* cfaTile /* [288*288] */, const float* maskR, const float* maskG, const float* maskB)` — assembles the planar NCHW `[CFA, R, G, B]` tensor.

- [ ] **Step 1: Add failing tests to `test_nn_preprocess.cpp`**

Append (before `std::cout << "test_nn_preprocess: OK\n";`):

```cpp
    // --- makeCanonicalMasks: Bayer RGGB ---
    {
        constexpr int N = NN_PATCH_SIZE * NN_PATCH_SIZE;
        std::vector<float> mr(N), mg(N), mb(N);
        CfaPhase rggb; rggb.period = 2; rggb.isXtrans = false;
        makeCanonicalMasks(mr.data(), mg.data(), mb.data(), rggb);
        // RGGB: (0,0)=R, (0,1)=G, (1,0)=G, (1,1)=B
        int idx00 = 0 * NN_PATCH_SIZE + 0;
        int idx01 = 0 * NN_PATCH_SIZE + 1;
        int idx10 = 1 * NN_PATCH_SIZE + 0;
        int idx11 = 1 * NN_PATCH_SIZE + 1;
        assert(mr[idx00] == 1.0f && mg[idx00] == 0.0f && mb[idx00] == 0.0f);
        assert(mr[idx01] == 0.0f && mg[idx01] == 1.0f && mb[idx01] == 0.0f);
        assert(mr[idx10] == 0.0f && mg[idx10] == 1.0f && mb[idx10] == 0.0f);
        assert(mr[idx11] == 0.0f && mg[idx11] == 0.0f && mb[idx11] == 1.0f);
        // Pattern repeats every 2
        assert(mr[(2 * NN_PATCH_SIZE + 0)] == 1.0f);  // row 2 == row 0
    }

    // --- packTileInput: channel order is [CFA, R, G, B] ---
    {
        constexpr int N = NN_PATCH_SIZE * NN_PATCH_SIZE;
        std::vector<float> cfa(N, 0.42f);
        std::vector<float> mr(N, 0.0f), mg(N, 1.0f), mb(N, 0.0f);
        std::vector<float> out(4 * N);
        packTileInput(out.data(), cfa.data(), mr.data(), mg.data(), mb.data());
        // Channel 0 = CFA
        assert(std::fabs(out[0] - 0.42f) < 1e-6f);
        // Channel 1 = R mask (all 0)
        assert(out[N] == 0.0f);
        // Channel 2 = G mask (all 1)
        assert(out[2 * N] == 1.0f);
        // Channel 3 = B mask (all 0)
        assert(out[3 * N] == 0.0f);
    }
```

Add `#include <vector>` at top of the test file.

- [ ] **Step 2: Run to verify failure**

```bash
cmake --build /tmp/ra-build-test --target test_nn_preprocess 2>&1 | tail -5
```
Expected: FAIL — `makeCanonicalMasks` / `packTileInput` / `NN_PATCH_SIZE` undefined.

- [ ] **Step 3: Add declarations to `nn_preprocess.h`**

Append before the closing `}`:

```cpp
/** Tile constants — fixed by the static ONNX export (design §2.5). */
static constexpr int NN_PATCH_SIZE = 288;
static constexpr int NN_OVERLAP = 48;
static constexpr int NN_STRIDE = NN_PATCH_SIZE - NN_OVERLAP;  // 240

/** Fill three NN_PATCH_SIZE × NN_PATCH_SIZE planes with one-hot masks of the
 *  CANONICAL CFA pattern (RGGB for Bayer, the standard 6×6 for X-Trans).
 *  The image is assumed already phase-aligned via mirror-pad using CfaPhase. */
void makeCanonicalMasks(float* outMaskR, float* outMaskG, float* outMaskB,
                        const CfaPhase& phase);

/** Assemble the [1,4,288,288] planar NCHW tile input from a CFA tile + 3 masks.
 *  Channel order: [CFA gray, R mask, G mask, B mask]. */
void packTileInput(float* outTile4ch,
                   const float* cfaTile,
                   const float* maskR,
                   const float* maskG,
                   const float* maskB);
```

- [ ] **Step 4: Implement in `nn_preprocess.cpp`**

Append:

```cpp
// Canonical X-Trans 6x6 pattern (Fujifilm standard). 0=R, 1=G, 2=B.
static const int XTRANS_CANONICAL[6][6] = {
    {1, 2, 1, 1, 0, 1},
    {0, 1, 0, 2, 1, 2},
    {1, 2, 1, 1, 0, 1},
    {1, 0, 1, 1, 2, 1},
    {2, 1, 2, 0, 1, 0},
    {1, 0, 1, 1, 2, 1}
};

void makeCanonicalMasks(float* outMaskR, float* outMaskG, float* outMaskB,
                        const CfaPhase& phase) {
    for (int y = 0; y < NN_PATCH_SIZE; ++y) {
        for (int x = 0; x < NN_PATCH_SIZE; ++x) {
            int idx = y * NN_PATCH_SIZE + x;
            int ch;
            if (phase.isXtrans) {
                ch = XTRANS_CANONICAL[y % 6][x % 6];
            } else {
                // Canonical RGGB: R=0,G=1,B=2 mapping to 2x2
                static const int RGGB_2x2[2][2] = {{0, 1}, {1, 2}};
                ch = RGGB_2x2[y % 2][x % 2];
            }
            outMaskR[idx] = (ch == 0) ? 1.0f : 0.0f;
            outMaskG[idx] = (ch == 1) ? 1.0f : 0.0f;
            outMaskB[idx] = (ch == 2) ? 1.0f : 0.0f;
        }
    }
}

void packTileInput(float* outTile4ch,
                   const float* cfaTile,
                   const float* maskR,
                   const float* maskG,
                   const float* maskB) {
    const int N = NN_PATCH_SIZE * NN_PATCH_SIZE;
    // Channel 0: CFA
    for (int i = 0; i < N; ++i) outTile4ch[i] = cfaTile[i];
    // Channel 1: R mask
    for (int i = 0; i < N; ++i) outTile4ch[N + i] = maskR[i];
    // Channel 2: G mask
    for (int i = 0; i < N; ++i) outTile4ch[2 * N + i] = maskG[i];
    // Channel 3: B mask
    for (int i = 0; i < N; ++i) outTile4ch[3 * N + i] = maskB[i];
}
```

- [ ] **Step 5: Run the test to verify it passes**

```bash
cmake --build /tmp/ra-build-test --target test_nn_preprocess
/tmp/ra-build-test/test_nn_preprocess
```
Expected: `test_nn_preprocess: OK`.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/lib/rawalchemy/include/nn_preprocess.h \
        src-tauri/lib/rawalchemy/src/nn_preprocess.cpp \
        src-tauri/lib/rawalchemy/Test/cpp/test_nn_preprocess.cpp
git commit -m "feat(nn): add canonical mask construction and 4-channel tile packing"
```

---

## Task 5: Postprocessing — Blend Weights & Color Matrix (TDD)

**Goal:** Trapezoidal blend window (design §2.4 step 1) and camRGB→sRGB matrix (design §2.4 step 3). Pure functions.

**Files:**
- Create: `src-tauri/lib/rawalchemy/include/nn_postprocess.h`
- Create: `src-tauri/lib/rawalchemy/src/nn_postprocess.cpp`
- Create: `src-tauri/lib/rawalchemy/Test/cpp/test_nn_postprocess.cpp`
- Modify: `src-tauri/lib/rawalchemy/CMakeLists.txt`

**Interfaces:**
- Produces:
  - `void makeTrapezoidWeights(float* outWeights2d /* [PATCH×PATCH] */)` — the 2D `wy*wx` weight window.
  - `void computeCamRgbToSrgb(float outMatrix[9], const float xyzToCam[9])` — `M = inv(xyzToCam @ inv(XYZ_TO_SRGB))`, rows normalized.
  - `void applyColorMatrixInPlace(float* rgb /* interleaved [H*W*3] */, size_t pixelCount, const float matrix[9])` — applies 3x3 matrix per pixel, low-side clamp only.

- [ ] **Step 1: Write failing tests**

Create `src-tauri/lib/rawalchemy/Test/cpp/test_nn_postprocess.cpp`:

```cpp
// SPDX-License-Identifier: AGPL-3.0-or-later
#include "../../include/nn_postprocess.h"

#include <cassert>
#include <cmath>
#include <iostream>

int main() {
    using namespace rawalchemy;

    // --- makeTrapezoidWeights ---
    {
        std::vector<float> w(NN_PATCH_SIZE * NN_PATCH_SIZE);
        makeTrapezoidWeights(w.data());
        // Corners (max ramp distance) should be smallest
        float corner = w[0];
        float center = w[(NN_PATCH_SIZE / 2) * NN_PATCH_SIZE + (NN_PATCH_SIZE / 2)];
        assert(corner < center);
        // Center should be 1.0 (flat region)
        assert(std::fabs(center - 1.0f) < 1e-6f);
        // Corner = (0/overlap) * (0/overlap) = 0
        assert(std::fabs(corner - 0.0f) < 1e-6f);
        // At x=overlap (just past ramp), weight in x should be 1.0
        float atRampEnd = w[(NN_PATCH_SIZE / 2) * NN_PATCH_SIZE + NN_OVERLAP];
        assert(std::fabs(atRampEnd - 1.0f) < 1e-6f);
    }

    // --- applyColorMatrixInPlace: identity matrix is a no-op ---
    {
        float identity[9] = {1,0,0, 0,1,0, 0,0,1};
        float rgb[] = {0.1f, 0.2f, 0.3f, 0.4f, 0.5f, 0.6f};
        applyColorMatrixInPlace(rgb, 2, identity);
        assert(std::fabs(rgb[0] - 0.1f) < 1e-6f);
        assert(std::fabs(rgb[5] - 0.6f) < 1e-6f);
    }

    // --- applyColorMatrixInPlace: low-side clamp ---
    {
        float zeroDiag[9] = {0,0,0, 0,0,0, 0,0,0};
        float rgb[] = {0.5f, 0.5f, 0.5f};
        applyColorMatrixInPlace(rgb, 1, zeroDiag);
        assert(rgb[0] == 0.0f);  // clamped, not negative
    }

    std::cout << "test_nn_postprocess: OK\n";
    return 0;
}
```

Add `#include <vector>` and `#include "nn_preprocess.h"` (for `NN_PATCH_SIZE`/`NN_OVERLAP`).

- [ ] **Step 2: Run to verify failure**

```bash
cmake --build /tmp/ra-build-test --target test_nn_postprocess 2>&1 | tail -5
```
Expected: FAIL — header not found.

- [ ] **Step 3: Create the header**

Create `src-tauri/lib/rawalchemy/include/nn_postprocess.h`:

```cpp
// SPDX-License-Identifier: AGPL-3.0-or-later
// Postprocessing primitives for x-veon NN demosaic.
// Algorithms: see docs/nn-demosaic-design.md §2.4.
#pragma once
#include <cstddef>
#include "nn_preprocess.h"  // for NN_PATCH_SIZE, NN_OVERLAP

namespace rawalchemy {

/** Fill a NN_PATCH_SIZE × NN_PATCH_SIZE buffer with the 2D trapezoidal blend
 *  weight window: w[y,x] = wy[y] * wx[x], where w1d ramps 0->1 over NN_OVERLAP
 *  px, is flat 1.0 in the center, and ramps 1->0 symmetrically. */
void makeTrapezoidWeights(float* outWeights2d);

/** Compute the camRGB -> sRGB 3x3 matrix from a camera's xyzToCam matrix.
 *  M = inv(xyzToCam @ inv(XYZ_TO_SRGB)), with xyzToCam rows normalized to sum=1.
 *  Output is row-major [3x3]. */
void computeCamRgbToSrgb(float outMatrix[9], const float xyzToCam[9]);

/** Apply a 3x3 color matrix (row-major) to interleaved RGB pixel buffer in place.
 *  Low-side clamps to 0.0f; high side NOT clamped (preserves HDR highlights). */
void applyColorMatrixInPlace(float* rgbInterleaved, size_t pixelCount, const float matrix[9]);

} // namespace rawalchemy
```

- [ ] **Step 4: Create the implementation**

Create `src-tauri/lib/rawalchemy/src/nn_postprocess.cpp`:

```cpp
// SPDX-License-Identifier: AGPL-3.0-or-later
#include "nn_postprocess.h"
#include <algorithm>
#include <cmath>

namespace rawalchemy {

void makeTrapezoidWeights(float* outWeights2d) {
    // Build 1D window first
    float w1d[NN_PATCH_SIZE];
    for (int i = 0; i < NN_PATCH_SIZE; ++i) {
        if (i < NN_OVERLAP) {
            w1d[i] = static_cast<float>(i) / static_cast<float>(NN_OVERLAP);
        } else if (i >= NN_PATCH_SIZE - NN_OVERLAP) {
            w1d[i] = static_cast<float>(NN_PATCH_SIZE - 1 - i) / static_cast<float>(NN_OVERLAP);
        } else {
            w1d[i] = 1.0f;
        }
    }
    // Outer product
    for (int y = 0; y < NN_PATCH_SIZE; ++y) {
        for (int x = 0; x < NN_PATCH_SIZE; ++x) {
            outWeights2d[y * NN_PATCH_SIZE + x] = w1d[y] * w1d[x];
        }
    }
}

// Standard sRGB-D65 -> XYZ (row-major), used to derive the inverse path.
static const float SRGB_TO_XYZ[9] = {
    0.4124564f, 0.3575761f, 0.1804375f,
    0.2126729f, 0.7151522f, 0.0721750f,
    0.0193339f, 0.1191920f, 0.9503041f
};

static void matmul3(const float a[9], const float b[9], float out[9]) {
    for (int r = 0; r < 3; ++r) {
        for (int c = 0; c < 3; ++c) {
            float sum = 0.0f;
            for (int k = 0; k < 3; ++k) sum += a[r * 3 + k] * b[k * 3 + c];
            out[r * 3 + c] = sum;
        }
    }
}

static bool invert3x3(const float m[9], float out[9]) {
    // Cofactor / determinant inversion
    float det = m[0] * (m[4] * m[8] - m[5] * m[7])
              - m[1] * (m[3] * m[8] - m[5] * m[6])
              + m[2] * (m[3] * m[7] - m[4] * m[6]);
    if (std::fabs(det) < 1e-20f) return false;
    float invDet = 1.0f / det;
    out[0] = (m[4] * m[8] - m[5] * m[7]) * invDet;
    out[1] = (m[2] * m[7] - m[1] * m[8]) * invDet;
    out[2] = (m[1] * m[5] - m[2] * m[4]) * invDet;
    out[3] = (m[5] * m[6] - m[3] * m[8]) * invDet;
    out[4] = (m[0] * m[8] - m[2] * m[6]) * invDet;
    out[5] = (m[2] * m[3] - m[0] * m[5]) * invDet;
    out[6] = (m[3] * m[7] - m[4] * m[6]) * invDet;
    out[7] = (m[1] * m[6] - m[0] * m[7]) * invDet;
    out[8] = (m[0] * m[4] - m[1] * m[3]) * invDet;
    return true;
}

void computeCamRgbToSrgb(float outMatrix[9], const float xyzToCam[9]) {
    // Normalize rows of xyzToCam to sum=1
    float normalized[9];
    for (int r = 0; r < 3; ++r) {
        float rowSum = xyzToCam[r * 3] + xyzToCam[r * 3 + 1] + xyzToCam[r * 3 + 2];
        float inv = (rowSum > 1e-20f) ? 1.0f / rowSum : 0.0f;
        for (int c = 0; c < 3; ++c) normalized[r * 3 + c] = xyzToCam[r * 3 + c] * inv;
    }
    float srgbToCam[9];
    matmul3(normalized, SRGB_TO_XYZ, srgbToCam);
    invert3x3(srgbToCam, outMatrix);  // = camRGB -> sRGB
}

void applyColorMatrixInPlace(float* rgb, size_t pixelCount, const float m[9]) {
    for (size_t i = 0; i < pixelCount; ++i) {
        float r = rgb[i * 3 + 0];
        float g = rgb[i * 3 + 1];
        float b = rgb[i * 3 + 2];
        float nr = m[0] * r + m[1] * g + m[2] * b;
        float ng = m[3] * r + m[4] * g + m[5] * b;
        float nb = m[6] * r + m[7] * g + m[8] * b;
        rgb[i * 3 + 0] = std::max(0.0f, nr);  // low-side clamp only
        rgb[i * 3 + 1] = std::max(0.0f, ng);
        rgb[i * 3 + 2] = std::max(0.0f, nb);
    }
}

} // namespace rawalchemy
```

- [ ] **Step 5: Register `src/nn_postprocess.cpp` in CMakeLists `raw_alchemy_core` sources (no fast-math override).**

- [ ] **Step 6: Run the test to verify it passes**

```bash
cmake --build /tmp/ra-build-test --target test_nn_postprocess
/tmp/ra-build-test/test_nn_postprocess
```
Expected: `test_nn_postprocess: OK`.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/lib/rawalchemy/include/nn_postprocess.h \
        src-tauri/lib/rawalchemy/src/nn_postprocess.cpp \
        src-tauri/lib/rawalchemy/Test/cpp/test_nn_postprocess.cpp \
        src-tauri/lib/rawalchemy/CMakeLists.txt
git commit -m "feat(nn): add trapezoidal blend weights and camRGB->sRGB color matrix"
```

---

## Task 6: ORT Session Singleton

**Goal:** Wrap ORT session lifecycle (Env, Session, EP registration) in a singleton. QNN HTP on Android, DirectML on Windows. This module has no unit test (ORT can't run without a real model file + EP); it is exercised by the integration test in Task 8.

**Files:**
- Create: `src-tauri/lib/rawalchemy/include/nn_session.h`
- Create: `src-tauri/lib/rawalchemy/src/nn_session.cpp`
- Modify: `src-tauri/lib/rawalchemy/CMakeLists.txt` (link ORT, conditional on `RA_ENABLE_NN_DEMOSAIC`)

**Interfaces:**
- Produces:
  - `enum class NnEpPreference { Auto };` (currently only Auto; reserved for future explicit EP control)
  - `struct NnSessionConfig { std::string bayerOnnxPath; std::string xtransOnnxPath; std::string qnnContextBinaryDir; /* Android only */ std::string directmlDllPath; /* Windows only */ NnEpPreference ep = NnEpPreference::Auto; }`
  - `class NnDemosaicSession` — singleton via `NnDemosaicSession::instance()`. Methods: `bool init(const NnSessionConfig&)`, `bool isReady() const`, `Ort::Session& bayerSession()`, `Ort::Session& xtransSession()`.
  - On init failure: returns false, `isReady()` stays false; caller falls back to classical demosaic (design §6.1).

- [ ] **Step 1: Create the header**

Create `src-tauri/lib/rawalchemy/include/nn_session.h`:

```cpp
// SPDX-License-Identifier: AGPL-3.0-or-later
// ORT session singleton for x-veon NN demosaic.
// EP selection: QNN HTP (Android, SD 8 Gen 2+) / DirectML (Windows).
// See docs/nn-demosaic-design.md §3.
#pragma once
#include <memory>
#include <string>

// Forward-declare ORT types to avoid leaking onnxruntime_cxx_api.h into callers.
namespace Ort { class Env; class Session; class MemoryInfo; }

namespace rawalchemy {

enum class NnEpPreference { Auto };

struct NnSessionConfig {
    std::string bayerOnnxPath;
    std::string xtransOnnxPath;
    std::string qnnContextBinaryDir;   // Android only
    std::string directmlDllPath;       // Windows only (app-local DirectML.dll)
    NnEpPreference ep = NnEpPreference::Auto;
};

class NnDemosaicSession {
public:
    static NnDemosaicSession& instance();

    /** Initialize ORT env + load both models + register EPs.
     *  Returns false on ANY failure (missing model file, EP init crash, DLL load fail).
     *  Idempotent: re-init after a failed init is allowed; re-init after success is a no-op. */
    bool init(const NnSessionConfig& config);

    bool isReady() const;

    /** Returns the session matching the CFA family (period 2 -> Bayer, else X-Trans).
     *  Returns nullptr if not ready. */
    Ort::Session* sessionForCfaPeriod(int period);

private:
    NnDemosaicSession();
    ~NnDemosaicSession();
    NnDemosaicSession(const NnDemosaicSession&) = delete;
    NnDemosaicSession& operator=(const NnDemosaicSession&) = delete;

    struct Impl;
    std::unique_ptr<Impl> impl_;
};

} // namespace rawalchemy
```

- [ ] **Step 2: Create the implementation**

Create `src-tauri/lib/rawalchemy/src/nn_session.cpp`. This is the platform-conditional EP registration. **The exact ORT C++ API calls for QNN HTP and DirectML are documented in the [ORT EP docs](https://onnxruntime.ai/docs/execution-providers/) — reference design §3.2/§3.3 for the specific options.**

```cpp
// SPDX-License-Identifier: AGPL-3.0-or-later
#include "nn_session.h"
#include <onnxruntime_cxx_api.h>
#include <iostream>

#ifdef _WIN32
  #include <windows.h>
  // Forward decl of the DML device creation we use to bypass System32 conflicts
  // (design §3.3, ORT issue #18831).
#endif

namespace rawalchemy {

struct NnDemosaicSession::Impl {
    std::unique_ptr<Ort::Env> env;
    std::unique_ptr<Ort::Session> bayer;
    std::unique_ptr<Ort::Session> xtrans;
    bool ready = false;
};

NnDemosaicSession& NnDemosaicSession::instance() {
    static NnDemosaicSession s;
    return s;
}
NnDemosaicSession::NnDemosaicSession() : impl_(std::make_unique<Impl>()) {}
NnDemosaicSession::~NnDemosaicSession() = default;

bool NnDemosaicSession::init(const NnSessionConfig& cfg) {
    if (impl_->ready) return true;
    try {
        // Single-threaded ORT (intra=inter=1); OpenMP parallelizes tiles above.
        Ort::ThreadingOptions topts;
        topts.SetIntraOpNumThreads(1);
        topts.SetInterOpNumThreads(1);
        impl_->env = std::make_unique<Ort::Env>(ORT_LOGGING_LEVEL_WARNING, "rawalchemy_nn", topts);

        auto makeSession = [&](const std::string& path) -> std::unique_ptr<Ort::Session> {
            Ort::SessionOptions sopts;
            sopts.SetIntraOpNumThreads(1);
            sopts.SetGraphOptimizationLevel(GraphOptimizationLevel::ORT_ENABLE_ALL);
#ifdef _WIN32
            // DirectML EP — load app-local DirectML.dll, create device, hand to ORT.
            // See design §3.3. Implementation note: call LoadLibraryA(cfg.directmlDllPath)
            // then DMLCreateDevice (imported via GetProcAddress), then
            // OrtSessionOptionsAppendExecutionProvider_DML(sopts, device, 0).
            // [IMPLEMENT on Windows build: see ORT directml.h sample]
#elif defined(__ANDROID__)
            // QNN HTP EP — enable_htp_fp16_precision="1", disable_cpu_ep_fallback="1",
            // retrieve cached context binary from cfg.qnnContextBinaryDir.
            // Implementation note: use OrtSessionOptionsAppendExecutionProvider_QNN
            // with backend_path="libQnnHtp.so", and set provider options:
            //   {"device_id","0"}, {"htp_performance_mode","burst"},
            //   {"enable_htp_fp16_precision","1"}, {"disable_cpu_ep_fallback","1"},
            //   {"qnn_context_binary_enable","true"},
            //   {"qnn_context_binary_path", cfg.qnnContextBinaryDir + "/<soc>.serialized.bin"}
            // [IMPLEMENT on Android build: see ORT qnn_ep.h sample]
#endif
            return std::make_unique<Ort::Session>(*impl_->env, path.c_str(), sopts);
        };

        impl_->bayer = makeSession(cfg.bayerOnnxPath);
        impl_->xtrans = makeSession(cfg.xtransOnnxPath);
        impl_->ready = true;
        return true;
    } catch (const Ort::Exception& e) {
        std::cerr << "NnDemosaicSession init failed: " << e.what() << std::endl;
        impl_->ready = false;
        impl_->bayer.reset();
        impl_->xtrans.reset();
        return false;
    } catch (const std::exception& e) {
        std::cerr << "NnDemosaicSession init failed (std): " << e.what() << std::endl;
        impl_->ready = false;
        return false;
    }
}

bool NnDemosaicSession::isReady() const { return impl_->ready; }

Ort::Session* NnDemosaicSession::sessionForCfaPeriod(int period) {
    if (!impl_->ready) return nullptr;
    return (period == 2) ? impl_->bayer.get() : impl_->xtrans.get();
}

} // namespace rawalchemy
```

> **Note to implementer:** The two `[IMPLEMENT on ... build]` blocks require filling in platform-specific ORT EP option bundles. The exact option keys and the LoadLibrary/DMLCreateDevice dance for Windows are in the [DirectML EP docs](https://onnxruntime.ai/docs/execution-providers/DirectML-ExecutionProvider.html) and [QNN EP docs](https://onnxruntime.ai/docs/execution-providers/QNN-ExecutionProvider.html). Do NOT guess — copy from the official samples linked there. This is the only step in the plan with platform-conditional ORT API usage.

- [ ] **Step 3: Link ORT in CMakeLists**

In `CMakeLists.txt`, inside the `if(RA_ENABLE_NN_DEMOSAIC)` block, after the ORT target imports, add to the `raw_alchemy_core` target (find the `target_link_libraries(raw_alchemy_core` block):

```cmake
if(RA_ENABLE_NN_DEMOSAIC)
    target_link_libraries(raw_alchemy_core PRIVATE onnxruntime::dynamic)
    target_compile_definitions(raw_alchemy_core PUBLIC RA_ENABLE_NN_DEMOSAIC=1)
    if(WIN32)
        # DirectML.dll is loaded at runtime via LoadLibrary; just record the path
        target_compile_definitions(raw_alchemy_core PRIVATE
            RA_DIRECTML_DLL_PATH="${DIRECTML_DLL_PATH}")
    endif()
endif()
```

Add `src/nn_session.cpp` to the `raw_alchemy_core` sources (guarded by `if(RA_ENABLE_NN_DEMOSAIC)`):

```cmake
if(RA_ENABLE_NN_DEMOSAIC)
    target_sources(raw_alchemy_core PRIVATE
        src/nn_session.cpp
        src/nn_preprocess.cpp   # already added in Task 3
        src/nn_postprocess.cpp  # already added in Task 5
        src/nn_nan_guard.cpp    # already added in Task 2
    )
endif()
```

(Adjust to append rather than duplicate if already added.)

- [ ] **Step 4: Verify it compiles on Windows**

```bash
cd /mnt/d/GitRepos/CameraFTP
cmake -S src-tauri/lib/rawalchemy -B /tmp/ra-win-build -DBUILD_CLI=ON -DBUILD_TESTS=OFF -DBUILD_SHARED=ON -DBUILD_CAPI=ON -RA_ENABLE_NN_DEMOSAIC=ON
cmake --build /tmp/ra-win-build --target raw_alchemy_core 2>&1 | tail -20
```
Expected: compiles. (The `[IMPLEMENT]` Windows block must be filled with the real DirectML calls first; if you build before that, expect an unresolved-symbol or logic-error — that's acceptable for this step as long as it compiles structurally.)

- [ ] **Step 5: Commit**

```bash
git add src-tauri/lib/rawalchemy/include/nn_session.h \
        src-tauri/lib/rawalchemy/src/nn_session.cpp \
        src-tauri/lib/rawalchemy/CMakeLists.txt
git commit -m "feat(nn): add ORT session singleton with QNN HTP / DirectML EP registration"
```

---

## Task 7: NN Demosaic Core — Tile Loop & Dispatch Entry Point

**Goal:** The `demosaic_nn_xveon.cpp` module that ties preprocessing + inference + postprocessing together. Exposes `nnDemosaic()` — the C++ entry point Plan B's FFI will call (Plan B adds the `enableNnDemosaic` FFI param that gates this).

**Files:**
- Create: `src-tauri/lib/rawalchemy/include/demosaic_nn_xveon.h`
- Create: `src-tauri/lib/rawalchemy/src/demosaic_nn_xveon.cpp`
- Modify: `src-tauri/lib/rawalchemy/CMakeLists.txt`

**Interfaces:**
- Consumes: `NnDemosaicSession` (Task 6), preprocessing (Tasks 3–4), postprocessing (Task 5), NaN guard (Task 2).
- Produces:
  - `enum class NnDemosaicStatus { Ok, SessionNotReady, NaNOutput, InferenceFailed, InvalidParam };`
  - `struct NnDemosaicInput { const float* cfaMosaic; int width; int height; unsigned filters; float blackLevel; float whiteLevel; const float wbRgb[3]; const float xyzToCam[9]; bool enableNn; }`
  - `struct NnDemosaicOutput { std::vector<float> rgbInterleaved; /* width*height*3 */ int width; int height; }`
  - `NnDemosaicStatus nnDemosaic(const NnDemosaicInput& in, NnDemosaicOutput& out)`

- [ ] **Step 1: Create the header**

Create `src-tauri/lib/rawalchemy/include/demosaic_nn_xveon.h`:

```cpp
// SPDX-License-Identifier: AGPL-3.0-or-later
// High-level NN demosaic entry point. Wraps the full x-veon pipeline:
// normalize -> WB -> phase-align -> tile -> infer (ORT) -> blend -> camRGB->sRGB.
// See docs/nn-demosaic-design.md §2 & §4.4.
#pragma once
#include <vector>
#include "cfa_lookup.h"

namespace rawalchemy {

enum class NnDemosaicStatus {
    Ok,
    SessionNotReady,   // NnDemosaicSession::init() not called or failed
    NaNOutput,         // Inference produced NaN/Inf — caller should report error
    InferenceFailed,   // ORT Run() threw
    InvalidParam       // null/zero-size input
};

struct NnDemosaicInput {
    const float* cfaMosaic;   // [width*height], normalized-or-raw, float
    int width;
    int height;
    unsigned filters;         // LibRaw CFA pattern code
    float blackLevel;         // single level (design §2.3 step 3)
    float whiteLevel;
    float wbRgb[3];           // R/G/B multipliers, G assumed ~1
    float xyzToCam[9];        // camera color matrix, row-major
};

struct NnDemosaicOutput {
    std::vector<float> rgbInterleaved;  // [width*height*3], linear sRGB, low-clamped
    int width = 0;
    int height = 0;
};

/** Run full NN demosaic. Returns Ok on success.
 *  - SessionNotReady: caller falls back to classical demosaic.
 *  - NaNOutput: caller reports error (no auto-fallback per design §6.2). */
NnDemosaicStatus nnDemosaic(const NnDemosaicInput& in, NnDemosaicOutput& out);

} // namespace rawalchemy
```

- [ ] **Step 2: Create the implementation skeleton (tile loop, no inference call yet)**

Create `src-tauri/lib/rawalchemy/src/demosaic_nn_xveon.cpp`. The structure: detect phase → normalize in-place copy → WB → mirror-pad → loop tiles (OpenMP) → blend → color matrix. The actual `session->Run()` call is inside the tile loop.

```cpp
// SPDX-License-Identifier: AGPL-3.0-or-later
#include "demosaic_nn_xveon.h"
#include "nn_session.h"
#include "nn_preprocess.h"
#include "nn_postprocess.h"
#include "nn_nan_guard.h"
#include <onnxruntime_cxx_api.h>
#include <algorithm>
#include <cmath>
#include <cstring>
#include <iostream>
#include <vector>

namespace rawalchemy {

namespace {
// Mirror-pad a 1D coordinate into [0, len).
inline int mirrorCoord(int v, int len) {
    if (len <= 0) return 0;
    int period = 2 * len;
    v = ((v % period) + period) % period;
    return (v < len) ? v : (period - 1 - v);
}

// Extract a 288x288 CFA tile from the (padded) mosaic starting at (ty*stride, tx*stride).
void extractCfaTile(const float* paddedCfa, int paddedW, int paddedH,
                    int originY, int originX, float* outTile) {
    for (int y = 0; y < NN_PATCH_SIZE; ++y) {
        for (int x = 0; x < NN_PATCH_SIZE; ++x) {
            int sy = std::min(originY + y, paddedH - 1);
            int sx = std::min(originX + x, paddedW - 1);
            outTile[y * NN_PATCH_SIZE + x] = paddedCfa[sy * paddedW + sx];
        }
    }
}
} // namespace

NnDemosaicStatus nnDemosaic(const NnDemosaicInput& in, NnDemosaicOutput& out) {
    if (!in.cfaMosaic || in.width <= 0 || in.height <= 0) {
        return NnDemosaicStatus::InvalidParam;
    }
    auto& session = NnDemosaicSession::instance();
    if (!session.isReady()) {
        return NnDemosaicStatus::SessionNotReady;
    }

    CfaPhase phase = detectCfaPhase(in.filters);
    Ort::Session* ortSession = session.sessionForCfaPeriod(phase.period);
    if (!ortSession) return NnDemosaicStatus::SessionNotReady;

    // --- Step 1: normalize (in a copy; do not mutate caller's buffer) ---
    const size_t pixelCount = static_cast<size_t>(in.width) * in.height;
    std::vector<float> cfa(in.cfaMosaic, in.cfaMosaic + pixelCount);
    normalizeCfaInPlace(cfa.data(), pixelCount, in.blackLevel, in.whiteLevel);

    // --- Step 2: per-site WB (design §2.3 step 5) ---
    // WB multipliers indexed by CFA color channel (0=R, 1=G, 2=B).
    const float wb[3] = { in.wbRgb[0], in.wbRgb[1], in.wbRgb[2] };
    for (int y = 0; y < in.height; ++y) {
        for (int x = 0; x < in.width; ++x) {
            int ch;
            if (phase.isXtrans) {
                // Use canonical X-Trans pattern (phase dy=dx=0 from LibRaw)
                extern const int XTRANS_CANONICAL[6][6];  // defined in nn_preprocess.cpp
                ch = XTRANS_CANONICAL[y % 6][x % 6];
            } else {
                int dy = (y + phase.dy) % 2;
                int dx = (x + phase.dx) % 2;
                static const int RGGB_2x2[2][2] = {{0, 1}, {1, 2}};
                ch = RGGB_2x2[dy][dx];
            }
            cfa[y * in.width + x] *= wb[ch];
        }
    }

    // --- Step 3: mirror-pad to align CFA phase + cover tile grid ---
    // Pad top by phase.dy, left by phase.dx, then out to a multiple of stride+patch.
    int padTop = phase.dy;
    int padLeft = phase.dx;
    int paddedW = ((in.width + padLeft + NN_STRIDE - 1) / NN_STRIDE) * NN_STRIDE + NN_OVERLAP;
    int paddedH = ((in.height + padTop + NN_STRIDE - 1) / NN_STRIDE) * NN_STRIDE + NN_OVERLAP;
    std::vector<float> paddedCfa(paddedW * paddedH, 0.0f);
    for (int y = 0; y < in.height; ++y) {
        for (int x = 0; x < in.width; ++x) {
            paddedCfa[(y + padTop) * paddedW + (x + padLeft)] = cfa[y * in.width + x];
        }
    }
    // Mirror-pad the borders (right and bottom) to fill paddedCfa fully.
    // For brevity the right/bottom mirror is elided here; implement by reflecting
    // rows/cols beyond in.width/in.height using mirrorCoord.

    // --- Step 4: build canonical masks (computed once) ---
    std::vector<float> maskR(NN_PATCH_SIZE * NN_PATCH_SIZE);
    std::vector<float> maskG(NN_PATCH_SIZE * NN_PATCH_SIZE);
    std::vector<float> maskB(NN_PATCH_SIZE * NN_PATCH_SIZE);
    makeCanonicalMasks(maskR.data(), maskG.data(), maskB.data(), phase);

    // --- Step 5: tile loop (OpenMP parallel) ---
    std::vector<float> outputAccum(paddedW * paddedH * 3, 0.0f);
    std::vector<float> weightAccum(paddedW * paddedH, 0.0f);
    std::vector<float> blendW(NN_PATCH_SIZE * NN_PATCH_SIZE);
    makeTrapezoidWeights(blendW.data());

    int tilesX = (paddedW - NN_OVERLAP) / NN_STRIDE;
    int tilesY = (paddedH - NN_OVERLAP) / NN_STRIDE;
    std::atomic<bool> nanDetected{false};
    std::atomic<bool> inferenceFailed{false};

    Ort::MemoryInfo memInfo = Ort::MemoryInfo::CreateCpu(OrtArenaAllocator, OrtMemTypeDefault);
    Ort::AllocatorWithDefaultOptions allocator;

    #pragma omp parallel for collapse(2) schedule(dynamic)
    for (int ty = 0; ty < tilesY; ++ty) {
        for (int tx = 0; tx < tilesX; ++tx) {
            if (nanDetected.load() || inferenceFailed.load()) continue;
            int originY = ty * NN_STRIDE;
            int originX = tx * NN_STRIDE;

            // Extract CFA tile + pack 4-channel input
            std::vector<float> cfaTile(NN_PATCH_SIZE * NN_PATCH_SIZE);
            extractCfaTile(paddedCfa.data(), paddedW, paddedH, originY, originX, cfaTile.data());
            std::vector<float> tileInput(4 * NN_PATCH_SIZE * NN_PATCH_SIZE);
            packTileInput(tileInput.data(), cfaTile.data(),
                          maskR.data(), maskG.data(), maskB.data());

            // Inference
            std::vector<float> tileOutput(3 * NN_PATCH_SIZE * NN_PATCH_SIZE);
            try {
                std::array<int64_t, 4> inShape = {1, 4, NN_PATCH_SIZE, NN_PATCH_SIZE};
                Ort::Value inputTensor = Ort::Value::CreateTensor<float>(
                    memInfo, tileInput.data(), tileInput.size(), inShape.data(), inShape.size());
                std::array<int64_t, 4> outShape = {1, 3, NN_PATCH_SIZE, NN_PATCH_SIZE};
                Ort::Value outputTensor = Ort::Value::CreateTensor<float>(
                    memInfo, tileOutput.data(), tileOutput.size(), outShape.data(), outShape.size());
                const char* inNames[] = {"input"};
                const char* outNames[] = {"output"};
                ortSession->Run(Ort::RunOptions{nullptr}, inNames, &inputTensor, 1,
                                outNames, &outputTensor, 1);
            } catch (...) {
                inferenceFailed.store(true);
                continue;
            }

            // NaN guard (fast-math-disabled function)
            if (nnOutputHasNaNInf(tileOutput.data(), tileOutput.size())) {
                nanDetected.store(true);
                continue;
            }

            // Accumulate weighted into the full output buffers
            for (int y = 0; y < NN_PATCH_SIZE; ++y) {
                for (int x = 0; x < NN_PATCH_SIZE; ++x) {
                    int gi = (originY + y) * paddedW + (originX + x);
                    float w = blendW[y * NN_PATCH_SIZE + x];
                    for (int c = 0; c < 3; ++c) {
                        outputAccum[gi * 3 + c] += tileOutput[c * NN_PATCH_SIZE * NN_PATCH_SIZE
                                                                + y * NN_PATCH_SIZE + x] * w;
                    }
                    weightAccum[gi] += w;
                }
            }
        }
    }

    if (nanDetected.load()) return NnDemosaicStatus::NaNOutput;
    if (inferenceFailed.load()) return NnDemosaicStatus::InferenceFailed;

    // --- Step 6: finalize (divide by weights) + crop padding ---
    out.width = in.width;
    out.height = in.height;
    out.rgbInterleaved.resize(static_cast<size_t>(in.width) * in.height * 3);
    for (int y = 0; y < in.height; ++y) {
        for (int x = 0; x < in.width; ++x) {
            int gi = (y + padTop) * paddedW + (x + padLeft);
            float w = weightAccum[gi];
            float invW = (w > 1e-20f) ? 1.0f / w : 0.0f;
            for (int c = 0; c < 3; ++c) {
                out.rgbInterleaved[(y * in.width + x) * 3 + c] =
                    outputAccum[gi * 3 + c] * invW;
            }
        }
    }

    // --- Step 7: camRGB -> sRGB matrix, low-side clamp ---
    float colorMatrix[9];
    computeCamRgbToSrgb(colorMatrix, in.xyzToCam);
    applyColorMatrixInPlace(out.rgbInterleaved.data(),
                            static_cast<size_t>(in.width) * in.height, colorMatrix);

    return NnDemosaicStatus::Ok;
}

} // namespace rawalchemy
```

> **Note to implementer:** The `extern const int XTRANS_CANONICAL[6][6]` linkage needs the symbol exported from `nn_preprocess.cpp` — change the `static` there to a non-static declaration in `nn_preprocess.h` if you reference it this way. Alternatively, move the canonical X-Trans pattern to a shared header. Pick one and keep both files consistent.

- [ ] **Step 3: Add to CMakeLists sources (under `RA_ENABLE_NN_DEMOSAIC`)**

- [ ] **Step 4: Verify it compiles**

```bash
cmake --build /tmp/ra-win-build --target raw_alchemy_core 2>&1 | tail -20
```
Expected: compiles. (No test yet — integration test is Task 8.)

- [ ] **Step 5: Commit**

```bash
git add src-tauri/lib/rawalchemy/include/demosaic_nn_xveon.h \
        src-tauri/lib/rawalchemy/src/demosaic_nn_xveon.cpp \
        src-tauri/lib/rawalchemy/CMakeLists.txt
git commit -m "feat(nn): implement x-veon tile-loop demosaic core with OpenMP + NaN guard"
```

---

## Task 8: Dispatch Router + Integration Test

**Goal:** Wire the NN path into `demosaic_dispatch.cpp` and write an integration test that exercises the full pipeline against a synthetic CFA fixture.

**Files:**
- Create: `src-tauri/lib/rawalchemy/include/demosaic_dispatch.h`
- Modify: `src-tauri/lib/rawalchemy/src/demosaic_dispatch.cpp`
- Create: `src-tauri/lib/rawalchemy/Test/cpp/test_nn_dispatch.cpp`
- Create: `src-tauri/lib/rawalchemy/Test/data/bayer_test_cfa.bin` (fixture generator script)
- Modify: `src-tauri/lib/rawalchemy/CMakeLists.txt`

**Interfaces:**
- Produces:
  - `enum class DemosaicPath { Classical, Neural };`
  - `NnDemosaicStatus demosaicDispatch(const NnDemosaicInput& in, NnDemosaicOutput& out, DemosaicPath path)` — routes to classical or NN. For `Neural`: if NN returns `SessionNotReady`, **does NOT auto-fallback** (returns the status; caller decides). For `NaNOutput`/`InferenceFailed`: returns as-is (no fallback per design §6.2).

- [ ] **Step 1: Create the dispatch header**

Create `src-tauri/lib/rawalchemy/include/demosaic_dispatch.h`:

```cpp
// SPDX-License-Identifier: AGPL-3.0-or-later
// Runtime router between classical (RCD/Markesteijn) and NN (x-veon) demosaic.
// See docs/nn-demosaic-design.md §4.4.
#pragma once
#include "demosaic_nn_xveon.h"

namespace rawalchemy {

enum class DemosaicPath { Classical, Neural };

/** Route to classical or NN demosaic.
 *  - Classical: delegates to existing RCD/Markesteijn (not yet wired in this header;
 *    the existing pipeline calls them directly — Plan B integrates the dispatch point).
 *  - Neural: calls nnDemosaic(). Returns its status verbatim; NO auto-fallback on
 *    NaN/InferenceFailed (caller reports error per design §6.2). SessionNotReady is
 *    also returned verbatim — the caller (FFI layer in Plan B) decides whether to
 *    retry with Classical. */
NnDemosaicStatus demosaicDispatch(const NnDemosaicInput& in, NnDemosaicOutput& out,
                                  DemosaicPath path);

} // namespace rawalchemy
```

- [ ] **Step 2: Implement the dispatch router**

Replace the contents of `src-tauri/lib/rawalchemy/src/demosaic_dispatch.cpp`:

```cpp
// SPDX-License-Identifier: AGPL-3.0-or-later
// Runtime router between classical and NN demosaic.
//
// The classical path (RCD for Bayer, Markesteijn for X-Trans) remains the default
// for preview and as the caller-chosen fallback when NN is unavailable or fails.
// Plan B (app integration) wires this into the raProcessFileWithLUT pipeline via
// the new enableNnDemosaic parameter.

#include "demosaic_dispatch.h"
#include "demosaic_rcd.h"
#include "demosaic_markesteijn.h"

namespace rawalchemy {

NnDemosaicStatus demosaicDispatch(const NnDemosaicInput& in, NnDemosaicOutput& out,
                                  DemosaicPath path) {
    if (path == DemosaicPath::Neural) {
        return nnDemosaic(in, out);
    }
    // Classical path: Plan B will route to RCD/Markesteijn here and populate `out`
    // from their output. For Plan A, we leave this as a stub returning an error so
    // that callers must explicitly choose Neural during the library-core phase.
    // (The classical demosaic functions consume a different in-memory representation
    // than NnDemosaicInput; the conversion happens in Plan B's pipeline integration.)
    return NnDemosaicStatus::InvalidParam;
}

} // namespace rawalchemy
```

- [ ] **Step 3: Generate the synthetic test fixture**

Create `src-tauri/lib/rawalchemy/Test/data/generate_fixture.py`:

```python
#!/usr/bin/env python3
"""Generate a synthetic 64x64 RGGB Bayer CFA fixture for unit/integration tests.
Output: bayer_test_cfa.bin (64*64 float32, raw-ish values in [0, 1023])."""
import struct, os
import numpy as np

W = H = 64
# Smooth gradient with a diagonal edge so demosaic has something to interpolate.
yy, xx = np.mgrid[0:H, 0:W].astype(np.float32)
cfa = (256 + 128 * np.sin(xx / 8.0) + 64 * yy / H) * 4  # ~[0, 2048]
cfa = cfa.astype(np.float32)

out_dir = os.path.dirname(os.path.abspath(__file__))
with open(os.path.join(out_dir, "bayer_test_cfa.bin"), "wb") as f:
    f.write(cfa.tobytes())
print(f"wrote {W}x{H} fixture")
```

Run it: `python3 src-tauri/lib/rawalchemy/Test/data/generate_fixture.py`

- [ ] **Step 4: Write the integration test**

Create `src-tauri/lib/rawalchemy/Test/cpp/test_nn_dispatch.cpp`. This test verifies the dispatch plumbing without requiring a real ORT session (which needs model files + EP). It asserts the SessionNotReady path and the InvalidParam path. A separate "golden image" test that requires the actual `.onnx` files is gated behind an env var so CI without models still passes.

```cpp
// SPDX-License-Identifier: AGPL-3.0-or-later
#include "../../include/demosaic_dispatch.h"
#include "../../include/nn_session.h"

#include <cassert>
#include <fstream>
#include <iostream>
#include <vector>

int main() {
    using namespace rawalchemy;

    // --- Test 1: Neural path without init returns SessionNotReady ---
    {
        std::vector<float> cfa(64 * 64, 512.0f);
        NnDemosaicInput in{};
        in.cfaMosaic = cfa.data();
        in.width = 64; in.height = 64;
        in.filters = 0x94949494u;  // RGGB
        in.blackLevel = 0.0f; in.whiteLevel = 1023.0f;
        in.wbRgb[0] = in.wbRgb[1] = in.wbRgb[2] = 1.0f;
        for (int i = 0; i < 9; ++i) in.xyzToCam[i] = (i % 4 == 0) ? 1.0f : 0.0f;  // identity-ish
        NnDemosaicOutput out;
        NnDemosaicStatus s = demosaicDispatch(in, out, DemosaicPath::Neural);
        assert(s == NnDemosaicStatus::SessionNotReady);
    }

    // --- Test 2: Invalid params ---
    {
        NnDemosaicInput in{};
        in.cfaMosaic = nullptr;
        in.width = 0; in.height = 0;
        NnDemosaicOutput out;
        NnDemosaicStatus s = demosaicDispatch(in, out, DemosaicPath::Neural);
        assert(s == NnDemosaicStatus::InvalidParam);
    }

    // --- Test 3 (gated): full golden-image demosaic, only if models present ---
    // Set RA_NN_INTEGRATION_TEST=1 and provide model paths via env to enable.
    {
        const char* enabled = std::getenv("RA_NN_INTEGRATION_TEST");
        const char* bayerPath = std::getenv("RA_NN_BAYER_ONNX");
        if (enabled && bayerPath) {
            NnSessionConfig cfg;
            cfg.bayerOnnxPath = bayerPath;
            cfg.xtransOnnxPath = bayerPath;  // reuse for test
            assert(NnDemosaicSession::instance().init(cfg) == true);
            assert(NnDemosaicSession::instance().isReady() == true);

            // Load fixture
            std::ifstream f("Test/data/bayer_test_cfa.bin", std::ios::binary);
            std::vector<float> cfa((std::istreambuf_iterator<char>(f)), {});
            cfa.resize(64 * 64);

            NnDemosaicInput in{};
            in.cfaMosaic = cfa.data();
            in.width = 64; in.height = 64;
            in.filters = 0x94949494u;
            in.blackLevel = 0.0f; in.whiteLevel = 2048.0f;
            in.wbRgb[0] = in.wbRgb[1] = in.wbRgb[2] = 1.0f;
            float identity[9] = {1,0,0, 0,1,0, 0,0,1};
            std::memcpy(in.xyzToCam, identity, sizeof(identity));
            NnDemosaicOutput out;
            NnDemosaicStatus s = demosaicDispatch(in, out, DemosaicPath::Neural);
            assert(s == NnDemosaicStatus::Ok);
            assert(out.width == 64 && out.height == 64);
            assert(out.rgbInterleaved.size() == 64 * 64 * 3);
            // NaN guard should have caught any bad output -> status would be NaNOutput
        } else {
            std::cout << "test_nn_dispatch: skipping integration (set RA_NN_INTEGRATION_TEST=1 + RA_NN_BAYER_ONNX=path)\n";
        }
    }

    std::cout << "test_nn_dispatch: OK\n";
    return 0;
}
```

Add `#include <cstring>` and `#include <cstdlib>` at the top.

- [ ] **Step 5: Register the new source files and test in CMakeLists**

Add `src/demosaic_dispatch.cpp` to `raw_alchemy_core` sources (it currently only includes headers). Add `test_nn_dispatch` to the test glob (automatic via the `file(GLOB RA_TEST_SOURCES ...)`).

- [ ] **Step 6: Build and run the non-integration tests**

```bash
cmake --build /tmp/ra-build-test --target test_nn_dispatch
/tmp/ra-build-test/test_nn_dispatch
```
Expected: prints `test_nn_dispatch: OK` (integration sub-test skipped).

- [ ] **Step 7: Commit**

```bash
git add src-tauri/lib/rawalchemy/include/demosaic_dispatch.h \
        src-tauri/lib/rawalchemy/src/demosaic_dispatch.cpp \
        src-tauri/lib/rawalchemy/Test/cpp/test_nn_dispatch.cpp \
        src-tauri/lib/rawalchemy/Test/data/generate_fixture.py \
        src-tauri/lib/rawalchemy/Test/data/bayer_test_cfa.bin \
        src-tauri/lib/rawalchemy/CMakeLists.txt
git commit -m "feat(nn): wire dispatch router + integration test scaffolding"
```

---

## Task 9: Vendor x-veon Models + Final Cross-Platform Build Verification

**Goal:** Commit the actual ONNX model files, then verify the full library builds on both Windows and Android with `RA_ENABLE_NN_DEMOSAIC=ON`.

**Files:**
- Create: `src-tauri/resources/models/xveon/bayer.onnx` (~4MB)
- Create: `src-tauri/resources/models/xveon/xtrans.onnx` (~15.5MB)
- Create: `src-tauri/resources/models/xveon/README.md`

- [ ] **Step 1: Obtain the models**

```bash
cd /mnt/d/GitRepos/CameraFTP
mkdir -p src-tauri/resources/models/xveon
# Clone x-veon to /tmp (outside workspace), extract the fp16 ONNX files.
git clone --depth 1 https://github.com/naorunaoru/x-veon.git /tmp/x-veon
cp /tmp/x-veon/web/public/bayer.onnx src-tauri/resources/models/xveon/bayer.onnx
cp /tmp/x-veon/web/public/xtrans.onnx src-tauri/resources/models/xveon/xtrans.onnx
ls -lh src-tauri/resources/models/xveon/
```
Expected: two files, ~4MB and ~15.5MB.

- [ ] **Step 2: Create a README documenting provenance + license**

Create `src-tauri/resources/models/xveon/README.md`:

```markdown
# x-veon Neural Demosaic Models

Source: https://github.com/naorunaoru/x-veon
Author: Roman Kuraev (naorunaoru)
License: [TODO: confirm with upstream — author stated intent to attach a permissive
license; verify before distribution]

- `bayer.onnx` — Bayer CFA demosaic, 1.95M params, opset 17, fp16, 46.04 dB PSNR
- `xtrans.onnx` — X-Trans CFA demosaic, 7.77M params, opset 17, fp16, 45.78 dB PSNR

See `docs/nn-demosaic-design.md` for the integration contract.
```

- [ ] **Step 3: Verify the full Windows build**

```bash
./build.sh windows
```
Expected: completes successfully. (If `RA_ENABLE_NN_DEMOSAIC` integration with the FFI build has issues, fix the CMake guard — FFI build is `BUILD_CLI=OFF BUILD_SHARED=ON BUILD_CAPI=ON`.)

- [ ] **Step 4: Verify the Android build**

```bash
./build.sh android
```
Expected: completes successfully. QNN runtime `.so`s are NOT yet packaged (Plan B adds `jniLibs` wiring) — this step only verifies the library compiles against the ORT Android headers.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/resources/models/xveon/
git commit -m "feat(nn): vendor x-veon Bayer + X-Trans ONNX models"
```

---

## Self-Review

**Spec coverage check (against `docs/nn-demosaic-design.md`):**
- §2 Model contract (I/O, opset, residual skip): ✓ Task 6 (session), Task 7 (uses the contract)
- §2.3 Preprocessing (7 steps): ✓ Task 3 (normalize, phase), Task 4 (masks, pack), Task 7 (WB, mirror-pad, tile loop)
- §2.4 Postprocessing (blend, color matrix, clamp): ✓ Task 5 (weights, matrix), Task 7 (finalize + matrix apply)
- §2.5 Tile params (288/48/240): ✓ Task 4 constants
- §3.1–3.3 EP selection (QNN HTP, DirectML): ✓ Task 6
- §4.1–4.5 C++ integration, fast-math isolation: ✓ Task 2 (NaN guard), Task 6/7 (core)
- §4.3 FFI signature change: ✗ DEFERRED to Plan B (per scope split — Plan A exposes internal `nnDemosaic()`, Plan B wires the FFI param)
- §4.4 Dispatch routing: ✓ Task 8
- §5 Resource packaging: ✓ Task 9 (models); build.sh resource extraction deferred to Plan B
- §6 Reliability (init fail → traditional, NaN → error): ✓ Task 6 (init fail returns false), Task 7 (NaN → NaNOutput status), Task 8 (dispatch returns status verbatim, no auto-fallback)
- §8 Implementation steps 1–8: ✓ Tasks 1–9 cover all
- **Gaps:** FFI signature change, Rust libloading, Kotlin whitelist, build.sh packaging — all explicitly deferred to Plan B.

**Placeholder scan:** The two `[IMPLEMENT on Windows/Android build]` blocks in Task 6 Step 2 are flagged with explicit instructions pointing to official ORT EP docs (not "TBD" — they have the exact option keys listed inline). The classical-path stub in Task 8 Step 2 returns InvalidParam with an explanatory comment — this is intentional for Plan A's scope, not a placeholder.

**Type consistency:**
- `NnDemosaicStatus` enum: defined in Task 7, used in Task 8 ✓
- `NnDemosaicInput`/`NnDemosaicOutput`: defined in Task 7, used in Task 8 ✓
- `NnSessionConfig`: defined in Task 6, used in Task 8's integration test ✓
- `CfaPhase`: defined in Task 3, used in Task 7 ✓
- `nnOutputHasNaNInf`: declared Task 2, called Task 7 ✓
- `DemosaicPath`: defined Task 8 ✓
- `XTRANS_CANONICAL`: defined as `static` in Task 4, referenced via `extern` in Task 7 — **flagged inconsistency**, implementer must promote to non-static in a shared header or move to `nn_preprocess.h`. Documented in Task 7 note.

---

## Execution Handoff

**Plan A complete and saved to `docs/superpowers/plans/2026-06-25-nn-demosaic-cpp-core.md`. Two execution options:**

**1. Subagent-Driven (recommended)** — I dispatch a fresh `@fixer` subagent per task, review between tasks, fast iteration. Best for this plan because Tasks 6–8 have platform-conditional code that benefits from per-task verification gates.

**2. Inline Execution** — Execute tasks in this session using executing-plans, batch execution with checkpoints.

**Plan B (App Integration)** will be written after Plan A's interfaces (`nnDemosaic`, `NnDemosaicSession`) are proven compiling, since it depends on the exact final signatures.

**Which execution approach for Plan A?**
