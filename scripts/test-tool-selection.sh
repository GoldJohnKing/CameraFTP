#!/bin/bash
# test-tool-selection.sh - 测试工具选择功能

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/build-common.sh"

echo "=========================================="
echo "  工具选择功能测试"
echo "=========================================="
echo ""

# 测试工具检测
echo "[测试 1] 工具检测"
echo "-------------------"
for tool in cargo java javac keytool; do
    printf "  %-10s" "$tool:"
    if cmd=$(get_tool_cmd "$tool"); then
        platform=$(get_tool_platform "$tool")
        echo "✓ $cmd ($platform)"
    else
        echo "✗ 未找到"
    fi
done

echo ""
echo "[测试 2] 路径检测"
echo "-------------------"

printf "  %-20s" "Windows SDK:"
if path=$(detect_windows_android_sdk); then
    echo "✓ $path"
else
    echo "✗ 未找到"
fi

printf "  %-20s" "Linux SDK:"
if path=$(detect_linux_android_sdk); then
    echo "✓ $path"
else
    echo "✗ 未找到"
fi

printf "  %-20s" "Windows Java:"
if path=$(detect_windows_java_home); then
    echo "✓ $path"
else
    echo "✗ 未找到"
fi

printf "  %-20s" "Linux Java:"
if path=$(detect_linux_java_home); then
    echo "✓ $path"
else
    echo "✗ 未找到"
fi

echo ""
echo "[测试 3] 环境变量"
echo "-------------------"
echo "  ANDROID_HOME=${ANDROID_HOME:-未设置}"
echo "  JAVA_HOME=${JAVA_HOME:-未设置}"
echo "  NDK_HOME=${NDK_HOME:-未设置}"

echo ""
echo "=========================================="
echo "  测试完成"
echo "=========================================="
