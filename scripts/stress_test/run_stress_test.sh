#!/bin/bash
# CameraFTP 压力测试启动脚本

echo "CameraFTP 压力测试工具"
echo "======================"
echo ""

# 检查 uv 是否安装
if ! command -v uv &> /dev/null; then
    echo "错误：未找到 uv 命令"
    echo "请先安装 uv: https://docs.astral.sh/uv/getting-started/installation/"
    exit 1
fi

echo "正在检查依赖..."
uv pip install pillow numpy pillow-heif 2>/dev/null

echo ""
echo "启动压力测试..."
echo "目标: 192.168.1.214:2121"
echo "按 Ctrl+C 停止测试"
echo ""

# 使用 uv 创建的虚拟环境中的 Python
.venv/bin/python ftp_stress_test.py
