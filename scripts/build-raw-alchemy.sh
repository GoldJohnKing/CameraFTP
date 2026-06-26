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

    # WSL2 has two issues that break Windows CMake/Ninja builds:
    #
    # 1. Timestamp propagation: file edits from WSL don't reliably update
    #    the Windows-visible mtime on /mnt/ drives (9P protocol limitation).
    #    Ninja uses mtime to decide whether to recompile, so it silently
    #    skips changed sources. Fix: force-update LastWriteTime via PowerShell.
    #
    # 2. Stale process locks: previous ninja/cmake processes may leave
    #    .ninja_log locked, causing "failed recompaction: Permission denied".
    #    Fix: kill stale processes before building; retry with clean state
    #    if the first attempt fails.
    #
    if grep -qi microsoft /proc/version 2>/dev/null; then
        # Kill stale build processes that may lock ninja state files
        cmd.exe /C "taskkill /f /im ninja.exe" > /dev/null 2>&1 || true
        cmd.exe /C "taskkill /f /im clang-cl.exe" > /dev/null 2>&1 || true

        # Force-update Windows-visible timestamps on all source files
        powershell.exe -NoProfile -Command \
            "Get-ChildItem -Path '$win_path\src','$win_path\include' -Recurse -Include '*.cpp','*.h','*.c' | ForEach-Object { \$_.LastWriteTime = Get-Date }" \
            > /dev/null 2>&1
    fi

    # Build with escalating retry: fast incremental → state clean → full clean
    local build_ok=false
    if cmd.exe /C "scripts\\build_windows.bat $build_type" 2>&1; then
        build_ok=true
    elif rm -f build-windows-dll/.ninja_log build-windows-dll/.ninja_deps && \
         cmd.exe /C "scripts\\build_windows.bat $build_type" 2>&1; then
        warn "Succeeded after clearing ninja state"
        build_ok=true
    else
        warn "Build failed, attempting full clean rebuild..."
        rm -rf build-windows-dll
        if cmd.exe /C "scripts\\build_windows.bat $build_type" 2>&1; then
            warn "Succeeded after full clean rebuild"
            build_ok=true
        fi
    fi
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
    local cmake_status=$?
    if [ $cmake_status -ne 0 ]; then
        error "RawAlchemyCpp CMake build FAILED (exit code $cmake_status). Aborting — check compile errors above."
        return 1
    fi
    cd - > /dev/null

    local so_path="$abs_dir/build-android-arm64/libraw_alchemy.so"
    if [ ! -f "$so_path" ]; then
        error "RawAlchemyCpp .so not found at expected path after successful cmake build"
        return 1
    fi
    # Verify the .so was actually rebuilt (modified within the last 60 seconds)
    local so_mtime
    so_mtime=$(stat -c %Y "$so_path" 2>/dev/null || stat -f %m "$so_path" 2>/dev/null || echo 0)
    local now
    now=$(date +%s)
    local age=$((now - so_mtime))
    if [ $age -gt 60 ]; then
        error "RawAlchemyCpp .so is STALE (modified ${age}s ago) — build may have used cache. Aborting."
        return 1
    fi
    success "RawAlchemyCpp .so built: $so_path"
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
