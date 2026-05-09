#!/bin/bash
# Build RawAlchemyCpp dynamic library for the current platform
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/build-common.sh"

RAWALCHEMY_DIR="${RAWALCHEMY_DIR:-$SCRIPT_DIR/../../RawAlchemyCpp}"

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

    cmd.exe /C "cd /D \"$(wslpath -w "$abs_dir")\" && scripts\\build_windows.bat $build_type"

    local dll_path="$abs_dir/build-windows-dll/bin/$build_type/raw_alchemy_core.dll"
    if [ -f "$dll_path" ]; then
        success "RawAlchemyCpp DLL built: $dll_path"
    else
        error "RawAlchemyCpp DLL not found at expected path"
        return 1
    fi
}

# Entry point
case "${1:-}" in
    windows)
        shift
        build_raw_alchemy_windows "${1:-Release}"
        ;;
    *)
        echo "Usage: $0 windows [Release|Debug]"
        exit 1
        ;;
esac
