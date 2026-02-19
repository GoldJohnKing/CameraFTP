#!/bin/bash
set -e

echo "==================================="
echo "Camera FTP Companion - Windows Build (Bundle)"
echo "==================================="
echo ""

# Windows cargo path
WINDOWS_CARGO="/mnt/c/Users/GoldJohnKing/.cargo/bin/cargo.exe"

if [ ! -f "$WINDOWS_CARGO" ]; then
    echo "ERROR: Windows cargo not found at $WINDOWS_CARGO"
    exit 1
fi

echo "Using Windows cargo: $WINDOWS_CARGO"
echo ""

# Get project paths
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

echo "Project directory: $SCRIPT_DIR"
echo ""

# Navigate to src-tauri
cd src-tauri
echo "Building Windows installer..."
echo ""

# Build using Windows cargo with bundling
"$WINDOWS_CARGO" tauri build --target x86_64-pc-windows-msvc

echo ""
echo "==================================="
echo "Build completed successfully!"
echo "==================================="
echo ""
echo "Output locations:"
echo "  - Executable: src-tauri/target/x86_64-pc-windows-msvc/release/camera-ftp-companion.exe"
echo "  - Installer:  src-tauri/target/x86_64-pc-windows-msvc/release/bundle/"