#!/bin/bash
# CameraFTP - A Cross-platform FTP companion for camera photo transfer
# Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Asserts version consistency across package.json, src-tauri/Cargo.toml,
# src-tauri/tauri.conf.json, the README.md badge and src-tauri/Cargo.lock.
# Exits 1 with a diff report on mismatch. Intended for CI and as the
# pre-flight check inside bump-version.sh.
#
# Main-guarded: sourcing this file (as bump-version.sh does) only defines
# read_versions(); the consistency check below runs when executed directly.

# Parses the current version out of the five canonical locations (run from
# the repo root). Sets the pkg/cargo/tauri/badge/lock globals.
read_versions() {
    pkg="$(sed -n 's/^  "version": "\(.*\)",$/\1/p' package.json | head -1)"
    cargo="$(sed -n 's/^version = "\(.*\)"$/\1/p' src-tauri/Cargo.toml | head -1)"
    # Same anchor (^  "version": ..., 2-space top-level key) as the
    # bump-version.sh rewrite — keeps read and write symmetric.
    tauri="$(sed -n 's/^  "version": "\(.*\)",$/\1/p' src-tauri/tauri.conf.json | head -1)"
    badge="$(sed -n 's/.*version-\([0-9][0-9.]*\)-blue.*/\1/p' README.md | head -1)"
    lock="$(sed -n '/^name = "cameraftp"$/{n;s/^version = "\(.*\)"$/\1/p;}' src-tauri/Cargo.lock)"
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
    set -euo pipefail

    SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    cd "$SCRIPT_DIR/.."

    read_versions

    if [[ -z "$pkg" ]]; then
        echo "ERROR: could not read version from package.json" >&2
        exit 1
    fi

    failed=0
    check() {
        local label="$1" actual="$2"
        if [[ "$actual" != "$pkg" ]]; then
            echo "MISMATCH ${label}: '${actual:-<not found>}' != package.json '${pkg}'" >&2
            failed=1
        fi
    }
    check "src-tauri/Cargo.toml" "$cargo"
    check "src-tauri/tauri.conf.json" "$tauri"
    check "README.md badge" "$badge"
    check "src-tauri/Cargo.lock (cameraftp)" "$lock"

    if [[ "$failed" -ne 0 ]]; then
        echo "Version drift detected. Fix with: ./scripts/bump-version.sh <X.Y.Z>" >&2
        exit 1
    fi
    echo "Versions consistent: $pkg (package.json / Cargo.toml / tauri.conf.json / README badge / Cargo.lock)"
fi
