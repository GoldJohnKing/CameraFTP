#!/bin/bash
# build.sh - 统一构建入口
set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

usage() {
    echo "用法: ./build.sh <command> [options]"
    echo ""
    echo "命令:"
    echo "  windows           构建 Windows 可执行文件 (release)"
    echo "  windows-debug     构建 Windows 可执行文件 (debug)"
    echo "  android           构建 Android APK (release)"
    echo "  frontend          仅构建前端"
    echo ""
    echo "示例:"
    echo "  ./build.sh windows"
    echo "  ./build.sh windows-debug"
    echo "  ./build.sh android"
    echo "  ./build.sh frontend"
    exit 1
}

# 检查命令
if [ $# -eq 0 ]; then
    usage
fi

COMMAND=$1
shift

case $COMMAND in
    windows)
        echo -e "${GREEN}构建 Windows 可执行文件 (release)...${NC}"
        ./build-windows.sh release
        ;;
    windows-debug)
        echo -e "${GREEN}构建 Windows 可执行文件 (debug)...${NC}"
        ./build-windows.sh debug
        ;;
    android)
        echo -e "${GREEN}构建 Android APK (release)...${NC}"
        ./build-android.sh release
        ;;
    frontend)
        echo -e "${GREEN}构建前端...${NC}"
        bun install --no-cache
        bun run build
        ;;
    *)
        echo -e "${RED}未知命令: $COMMAND${NC}"
        usage
        ;;
esac
