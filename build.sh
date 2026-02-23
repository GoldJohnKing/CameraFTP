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
    echo "  windows           构建 Windows 可执行文件"
    echo "  windows-bundle    构建 Windows 安装包 (EXE + MSI)"
    echo "  android           构建 Android APK (debug)"
    echo "  android-release   构建 Android APK (release)"
    echo "  android-aab       构建 Android AAB (Google Play)"
    echo "  dev               启动开发模式"
    echo "  frontend          仅构建前端"
    echo ""
    echo "示例:"
    echo "  ./build.sh windows"
    echo "  ./build.sh android-release"
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
        echo -e "${GREEN}构建 Windows 可执行文件...${NC}"
        ./build-full.sh
        ;;
    windows-bundle)
        echo -e "${GREEN}构建 Windows 安装包...${NC}"
        ./build-windows-bundle.sh
        ;;
    android)
        echo -e "${GREEN}构建 Android APK (debug)...${NC}"
        ./build-android.sh debug
        ;;
    android-release)
        echo -e "${GREEN}构建 Android APK (release)...${NC}"
        ./build-android.sh release
        ;;
    android-aab)
        echo -e "${GREEN}构建 Android AAB...${NC}"
        ./build-android.sh aab
        ;;
    dev)
        echo -e "${GREEN}启动开发模式...${NC}"
        cargo tauri dev
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
