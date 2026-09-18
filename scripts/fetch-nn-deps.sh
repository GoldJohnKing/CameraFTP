#!/bin/bash
# CameraFTP - A Cross-platform FTP companion for camera photo transfer
# Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
# SPDX-License-Identifier: AGPL-3.0-or-later

# Fetches ONNX Runtime + DirectML (Windows) or ORT-android + QNN runtime (Android)
# into a vendored cache dir. Idempotent. Called by build.sh or manually.
set -euo pipefail

# Default cache dir is resolved script-relative so it matches where CMake looks:
# rawalchemy's CMakeLists.txt uses ${CMAKE_SOURCE_DIR}/third_party/nn-cache,
# where CMAKE_SOURCE_DIR is the rawalchemy submodule root. A CWD-relative default
# (the old behavior) would diverge from CMake when this script is run from the
# parent repo root rather than the submodule dir.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RA_ROOT="$(cd "${SCRIPT_DIR}/../src-tauri/lib/rawalchemy" && pwd)"
CACHE_DIR="${1:-${RA_ROOT}/third_party/nn-cache}"
mkdir -p "$CACHE_DIR"

# Versions come from the shared nn-versions.env (also consumed by build.sh /
# build-android.sh so cache dir names stay in sync).
source "$SCRIPT_DIR/nn-versions.env"
ORT_VERSION="$NN_DEMOSAIC_VERSION"
DIRECTML_VERSION="1.15.4"     # Microsoft.AI.DirectML NuGet (Windows-only)

SHA256_MANIFEST="$SCRIPT_DIR/nn-deps.sha256"

# Verify a downloaded artifact against scripts/nn-deps.sha256 (exact-URL match).
# Exits 1 on mismatch or missing entry — never extract an unverified archive.
verify_sha256() {
    local file="$1" url="$2"
    local expected
    expected="$(awk -v u="$url" '$2 == u { print $1; exit }' "$SHA256_MANIFEST")"
    if [[ -z "$expected" ]]; then
        echo "ERROR: no sha256 entry for $url in $SHA256_MANIFEST" >&2
        echo "       Append one: curl -sL '$url' | sha256sum" >&2
        exit 1
    fi
    local actual
    actual="$(sha256sum "$file" | awk '{print $1}')"
    if [[ "$actual" != "$expected" ]]; then
        echo "ERROR: sha256 mismatch for $url" >&2
        echo "  expected: $expected" >&2
        echo "  actual:   $actual" >&2
        exit 1
    fi
    echo "sha256 OK: $(basename "$file")"
}

# --- Windows ORT (DirectML-capable) + DirectML.dll ---
#
# The generic ORT GitHub release (onnxruntime-win-x64-<ver>.zip) has NO DirectML
# EP compiled in: GetExecutionProviderApi("DML",...) derefs null at runtime and NN
# init always fails. We instead pull Microsoft.ML.OnnxRuntime.DirectML — the same
# ORT version built with the DML EP. It is a .nupkg (zip) laid out as:
#   runtimes/win-x64/native/onnxruntime.dll   (DirectML-capable, ~17 MB)
#   runtimes/win-x64/native/onnxruntime.lib   (import lib, resolves OrtGetApiBase)
#   build/native/include/*.h                  (incl. dml_provider_factory.h)
# DirectML.dll itself is NOT bundled here — it ships in the separate
# Microsoft.AI.DirectML NuGet fetched below (the DML ORT loads it at runtime).
ORT_WIN_DIR="$CACHE_DIR/onnxruntime-win-x64-$ORT_VERSION"
# A stamp distinguishes the DirectML build from a previously-extracted generic
# ORT at the same path, forcing a re-extract when switching sources.
ORT_DIRECTML_VERSION="$ORT_VERSION"
if [[ ! -f "$ORT_WIN_DIR/.directml-stamp" ]]; then
    ORT_DML_URL="https://www.nuget.org/api/v2/package/Microsoft.ML.OnnxRuntime.DirectML/$ORT_DIRECTML_VERSION"
    curl -fL "$ORT_DML_URL" -o "$CACHE_DIR/onnxruntime-directml.nupkg.zip"
    verify_sha256 "$CACHE_DIR/onnxruntime-directml.nupkg.zip" "$ORT_DML_URL"
    mkdir -p "$ORT_WIN_DIR/lib" "$ORT_WIN_DIR/include"
    # Extract the native DLL + import lib into lib/ (matches the layout CMake's
    # ${RA_ORT_ROOT}/lib expects, identical to the generic GitHub release).
    unzip -j -o "$CACHE_DIR/onnxruntime-directml.nupkg.zip" \
        "runtimes/win-x64/native/onnxruntime.dll" \
        "runtimes/win-x64/native/onnxruntime.lib" \
        -d "$ORT_WIN_DIR/lib"
    # Headers (onnxruntime_cxx_api.h etc. + dml_provider_factory.h) into include/.
    unzip -j -o "$CACHE_DIR/onnxruntime-directml.nupkg.zip" \
        "build/native/include/*" \
        -d "$ORT_WIN_DIR/include" 2>/dev/null || true
    touch "$ORT_WIN_DIR/.directml-stamp"
fi
if [[ ! -f "$CACHE_DIR/DirectML.dll" ]]; then
    # Pull DirectML.dll from the Microsoft.AI.DirectML NuGet package
    DIRECTML_URL="https://www.nuget.org/api/v2/package/Microsoft.AI.DirectML/$DIRECTML_VERSION"
    curl -fL "$DIRECTML_URL" -o "$CACHE_DIR/directml.nupkg.zip"
    verify_sha256 "$CACHE_DIR/directml.nupkg.zip" "$DIRECTML_URL"
    unzip -qo -j "$CACHE_DIR/directml.nupkg.zip" "bin/x64-win/DirectML.dll" -d "$CACHE_DIR"
fi

# --- Linux ORT (x64) — dev-loop compilation only (CPU EP, no GPU EP wired) ---
if [[ ! -d "$CACHE_DIR/onnxruntime-linux-x64-$ORT_VERSION" ]]; then
    ORT_LINUX_URL="https://github.com/microsoft/onnxruntime/releases/download/v$ORT_VERSION/onnxruntime-linux-x64-$ORT_VERSION.tgz"
    curl -fL "$ORT_LINUX_URL" -o "$CACHE_DIR/onnxruntime-linux-x64-$ORT_VERSION.tgz"
    verify_sha256 "$CACHE_DIR/onnxruntime-linux-x64-$ORT_VERSION.tgz" "$ORT_LINUX_URL"
    tar -xzf "$CACHE_DIR/onnxruntime-linux-x64-$ORT_VERSION.tgz" -C "$CACHE_DIR"
fi

# --- Android ORT (arm64, QNN EP compiled in) + QNN runtime ---
#
# The generic onnxruntime-android AAR has NO QNN execution provider compiled
# in: AppendExecutionProvider("QNN", opts) fails because the QNN provider
# factory doesn't exist in the build (Android requires QNN statically linked,
# no plugin). We instead pull onnxruntime-android-qnn — the same ORT version
# built with the QNN EP statically linked in.
if [[ ! -d "$CACHE_DIR/onnxruntime-android-qnn-$ORT_VERSION" ]]; then
    ORT_QNN_URL="https://repo1.maven.org/maven2/com/microsoft/onnxruntime/onnxruntime-android-qnn/$ORT_VERSION/onnxruntime-android-qnn-$ORT_VERSION.aar"
    curl -fL "$ORT_QNN_URL" -o "$CACHE_DIR/ort-android-qnn.aar"
    verify_sha256 "$CACHE_DIR/ort-android-qnn.aar" "$ORT_QNN_URL"
    mkdir -p "$CACHE_DIR/onnxruntime-android-qnn-$ORT_VERSION"
    unzip -qo "$CACHE_DIR/ort-android-qnn.aar" -d "$CACHE_DIR/onnxruntime-android-qnn-$ORT_VERSION"
fi
if [[ ! -d "$CACHE_DIR/qnn-runtime-$QNN_RUNTIME_VERSION" ]]; then
    QNN_URL="https://repo1.maven.org/maven2/com/qualcomm/qti/qnn-runtime/$QNN_RUNTIME_VERSION/qnn-runtime-$QNN_RUNTIME_VERSION.aar"
    curl -fL "$QNN_URL" -o "$CACHE_DIR/qnn-runtime.aar"
    verify_sha256 "$CACHE_DIR/qnn-runtime.aar" "$QNN_URL"
    mkdir -p "$CACHE_DIR/qnn-runtime-$QNN_RUNTIME_VERSION"
    unzip -qo "$CACHE_DIR/qnn-runtime.aar" -d "$CACHE_DIR/qnn-runtime-$QNN_RUNTIME_VERSION"
fi

echo "NN deps cached in $CACHE_DIR"
