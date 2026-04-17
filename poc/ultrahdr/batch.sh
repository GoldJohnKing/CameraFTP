#!/bin/bash
# Batch convert all NEF files in hdr_example to Ultra HDR JPEG.
#
# CameraFTP - A Cross-platform FTP companion for camera photo transfer
# Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
# SPDX-License-Identifier: AGPL-3.0-or-later

set -euo pipefail

readonly SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
readonly VENV="$SCRIPT_DIR/.venv"
readonly INPUT_DIR="${1:-hdr_example}"

if [ ! -d "$INPUT_DIR" ]; then
    echo "Error: Directory '$INPUT_DIR' not found" >&2
    echo "Usage: $0 [input_directory]" >&2
    exit 1
fi

# Find NEF files (case-insensitive)
nef_files=()
while IFS= read -r -d '' f; do
    nef_files+=("$f")
done < <(find "$INPUT_DIR" -maxdepth 1 -iname '*.nef' -print0)

if [ ${#nef_files[@]} -eq 0 ]; then
    echo "No NEF files found in '$INPUT_DIR'"
    exit 0
fi

echo "Found ${#nef_files[@]} NEF file(s) in '$INPUT_DIR'"
echo

source "$VENV/bin/activate"

success=0
failed=0

for nef in "${nef_files[@]}"; do
    base="${nef%.*}"
    output="${base}_UltraHDR.jpg"

    echo "─────────────────────────────────────────────────────"
    echo "Converting: $(basename "$nef")"
    echo "Output:     $(basename "$output")"
    echo

    if python3 "$SCRIPT_DIR/main.py" --half-size "$nef" -o "$output"; then
        ((success++))
    else
        echo "  ✗ Failed"
        ((failed++))
    fi
    echo
done

echo "─────────────────────────────────────────────────────"
echo "Done: ${success} succeeded, ${failed} failed"
