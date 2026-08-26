#!/bin/bash
# 前端构建脚本

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/build-common.sh"

if [ "${FRONTEND_ALREADY_BUILT:-}" = "1" ]; then
    exit 0
fi

task "构建前端..."

bun install
bun run build
