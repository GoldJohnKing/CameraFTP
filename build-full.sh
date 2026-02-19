#!/bin/bash
set -e

echo "==================================="
echo "Camera FTP Companion - Full Build"
echo "==================================="
echo ""

# Windows paths
WINDOWS_CARGO="/mnt/c/Users/GoldJohnKing/.cargo/bin/cargo.exe"
WINDOWS_PATH="C:\\Users\\GoldJohnKing\\.cargo\\bin"

if [ ! -f "$WINDOWS_CARGO" ]; then
    echo "ERROR: Windows cargo not found at $WINDOWS_CARGO"
    exit 1
fi

echo "Step 1: Installing frontend dependencies..."
bun install --no-cache

echo ""
echo "Step 2: Building frontend..."
bun run build

echo ""
echo "Step 3: Building Windows executable with embedded frontend..."
cd src-tauri

# Use Windows cargo directly
"$WINDOWS_CARGO" build --release --target x86_64-pc-windows-msvc

echo ""
echo "==================================="
echo "Build completed successfully!"
echo "==================================="
echo ""
echo "Output:"
echo "  src-tauri/target/x86_64-pc-windows-msvc/release/camera-ftp-companion.exe"
echo ""
echo "This executable includes:"
echo "  - React frontend (embedded in the EXE)"
echo "  - Rust FTP server backend"
echo "  - All assets and resources"
echo ""
echo "Simply copy the EXE file to any Windows machine and run it!"