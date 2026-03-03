#!/bin/bash
# 前端构建脚本

set -e

# 跳过已构建的前端
if [ "${FRONTEND_ALREADY_BUILT:-}" = "1" ]; then
    echo "前端已构建，跳过"
    exit 0
fi

echo "构建前端..."
bun install
bun run build
