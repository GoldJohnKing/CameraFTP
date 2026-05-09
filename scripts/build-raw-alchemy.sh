#!/bin/bash
# Build RawAlchemyCpp dynamic library for the current platform
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/build-common.sh"

RAWALCHEMY_DIR="${RAWALCHEMY_DIR:-$SCRIPT_DIR/../src-tauri/lib/rawalchemy}"

build_raw_alchemy_windows() {
    local build_type="${1:-Release}"

    if [ ! -d "$RAWALCHEMY_DIR" ]; then
        warn "RawAlchemyCpp not found at $RAWALCHEMY_DIR"
        warn "Skipping RawAlchemyCpp build. LUT filter will not be available."
        warn "Set RAWALCHEMY_DIR to the RawAlchemyCpp directory to enable it."
        return 0
    fi

    task "[RawAlchemyCpp] Building Windows DLL ($build_type)..."

    local abs_dir
    abs_dir="$(cd "$RAWALCHEMY_DIR" && pwd)"

    local win_path
    win_path="$(wslpath -w "$abs_dir")"

    cd "$abs_dir"
    cmd.exe /C "scripts\\build_windows.bat $build_type"
    cd - > /dev/null

    local dll_path="$abs_dir/build-windows-dll/bin/$build_type/raw_alchemy_core.dll"
    if [ -f "$dll_path" ]; then
        success "RawAlchemyCpp DLL built: $dll_path"
    else
        error "RawAlchemyCpp DLL not found at expected path"
        return 1
    fi
}

build_raw_alchemy_android() {
    local build_type="${1:-Release}"

    if [ ! -d "$RAWALCHEMY_DIR" ]; then
        warn "RawAlchemyCpp not found at $RAWALCHEMY_DIR"
        warn "Skipping RawAlchemyCpp build. LUT filter will not be available."
        warn "Set RAWALCHEMY_DIR to the RawAlchemyCpp directory to enable it."
        return 0
    fi

    # Resolve NDK path
    local ndk_path="${NDK_HOME:-}"
    if [ -z "$ndk_path" ] && [ -d "${ANDROID_HOME:-}/ndk" ]; then
        for ndk_version in "${ANDROID_HOME}/ndk"/*; do
            if [ -d "$ndk_version" ]; then
                ndk_path="$ndk_version"
                break
            fi
        done
    fi

    if [ -z "$ndk_path" ] || [ ! -d "$ndk_path" ]; then
        warn "Android NDK not found. Skipping RawAlchemyCpp Android build."
        return 0
    fi

    task "[RawAlchemyCpp] Building Android arm64 .so ($build_type)..."

    local abs_dir
    abs_dir="$(cd "$RAWALCHEMY_DIR" && pwd)"

    cd "$abs_dir"
    ANDROID_NDK="$ndk_path" cmake -B "build-android-arm64" \
        -DCMAKE_TOOLCHAIN_FILE="$ndk_path/build/cmake/android.toolchain.cmake" \
        -DANDROID_ABI=arm64-v8a \
        -DANDROID_PLATFORM=android-33 \
        -DCMAKE_BUILD_TYPE="$build_type" \
        -DBUILD_SHARED=ON \
        -DBUILD_CAPI=ON \
        -DBUILD_CLI=OFF \
        -DENABLE_LENS_CORRECTION=ON \

    cmake --build "build-android-arm64" -j"$(nproc 2>/dev/null || echo 4)"
    cd - > /dev/null

    local so_path="$abs_dir/build-android-arm64/libraw_alchemy.so"
    if [ -f "$so_path" ]; then
        success "RawAlchemyCpp .so built: $so_path"
    else
        error "RawAlchemyCpp .so not found at expected path"
        return 1
    fi
}

# Entry point
case "${1:-}" in
    windows)
        shift
        build_raw_alchemy_windows "${1:-Release}"
        ;;
    android)
        shift
        build_raw_alchemy_android "${1:-Release}"
        ;;
    *)
        echo "Usage: $0 windows|android [Release|Debug]"
        exit 1
        ;;
esac
