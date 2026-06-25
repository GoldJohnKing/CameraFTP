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
