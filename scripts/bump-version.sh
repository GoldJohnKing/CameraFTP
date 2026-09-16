#!/bin/bash
# CameraFTP - A Cross-platform FTP companion for camera photo transfer
# Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Usage: ./scripts/bump-version.sh <X.Y.Z>
# Updates the version in ALL FOUR canonical files (see AGENTS.md), refreshes
# src-tauri/Cargo.lock via gen-types, then prints the new values for review.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR/.."

# Reuse the shared parsers (check-versions.sh is main-guarded: sourcing it
# defines read_versions() without running the consistency check).
source "$SCRIPT_DIR/check-versions.sh"

NEW_VERSION="${1:-}"
if [[ ! "$NEW_VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "ERROR: invalid or missing version '${NEW_VERSION}' (expected X.Y.Z, e.g. 1.9.3)" >&2
    exit 1
fi

read_versions
OLD_VERSION="$pkg"
if [[ -z "$OLD_VERSION" ]]; then
    echo "ERROR: failed to read current version from package.json" >&2
    exit 1
fi
if [[ "$OLD_VERSION" == "$NEW_VERSION" ]]; then
    echo "Version already $NEW_VERSION — nothing to do"
    exit 0
fi

# Fail fast on pre-existing drift between the canonical files
./scripts/check-versions.sh

echo "Bumping version: $OLD_VERSION -> $NEW_VERSION"

sed -i "s/^  \"version\": \"$OLD_VERSION\",/  \"version\": \"$NEW_VERSION\",/" package.json
sed -i "s/^version = \"$OLD_VERSION\"/version = \"$NEW_VERSION\"/" src-tauri/Cargo.toml
# Anchored to the top-level key (2-space indent): a nested "version" key —
# e.g. a future updater config — must not be rewritten by this bump.
sed -i "s/^  \"version\": \"$OLD_VERSION\"/  \"version\": \"$NEW_VERSION\"/" src-tauri/tauri.conf.json
sed -i "s/version-$OLD_VERSION-blue/version-$NEW_VERSION-blue/" README.md

# Refresh src-tauri/Cargo.lock's cameraftp entry (runs cargo.exe via build.sh)
./build.sh gen-types

echo ""
echo "Updated version references:"
read_versions
echo "  package.json              : $pkg"
echo "  src-tauri/Cargo.toml      : $cargo"
echo "  src-tauri/tauri.conf.json : $tauri"
echo "  README.md badge           : version-$badge-blue"
echo "  src-tauri/Cargo.lock      : $lock"
