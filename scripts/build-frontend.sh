#!/bin/bash
# build-frontend.sh - 前端构建脚本
# 支持通过环境变量跳过已构建的前端

# 如果前端已由 build.sh 统一构建，则跳过
if [ "${FRONTEND_ALREADY_BUILT}" = "1" ]; then
    echo "[build-frontend.sh] 前端已由 build.sh 统一构建，跳过..."
    exit 0
fi

# 正常构建前端
echo "[build-frontend.sh] 构建前端..."
bun install
bun run build
