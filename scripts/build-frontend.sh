#!/bin/bash
# build-frontend.sh - 前端资源构建脚本
# 编译 React 前端代码，生成静态资源文件

set -e

# 检查环境变量: 前端已由 build.sh 统一构建则跳过，避免重复编译
if [ "${FRONTEND_ALREADY_BUILT:-}" = "1" ]; then
    echo "[build-frontend.sh] 检测到前端已完成构建，跳过本次编译"
    exit 0
fi

# 安装依赖并构建前端
echo "[build-frontend.sh] 正在构建前端资源..."
bun install
bun run build
