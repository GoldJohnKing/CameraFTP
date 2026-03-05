#!/usr/bin/env python3
"""
处理捐赠二维码图片
- 移除顶部和底部的蓝绿色外框
- 保留中间二维码和下方白色背景的支付标识
- 压缩输出
"""

from PIL import Image
import os


def process_qrcode():
    input_path = "donation/unified.png"
    output_path = "public/donate-qrcode.png"

    # 打开图片
    img = Image.open(input_path)
    width, height = img.size
    print(f"原始图片尺寸: {width}x{height}")

    # 根据观察，图片结构大致如下：
    # - 顶部约 15% 是蓝色/绿色背景 + 支付图标
    # - 中间约 55% 是白色背景 + 二维码
    # - 二维码下方约 15% 是白色背景 + 支付图标
    # - 底部约 15% 是蓝色/绿色背景 + "欢迎光临"

    # 估算裁剪区域（基于图片比例）
    # 顶部裁剪 - 需要把顶部的蓝绿色背景完全去掉，从白色区域开始
    # 从图片观察，白色圆角矩形的顶部约在 26% 位置，再往里一点确保去除脏像素
    top_crop = int(height * 0.265)  # 约 26.5% 顶部，去掉蓝绿色背景和脏像素

    # 底部裁剪到支付标识下方（白色区域底部约在 83% 位置）
    bottom_crop = int(height * 0.825)  # 约 82.5% 处，去除底部脏像素

    # 左右两侧也有蓝绿色边，需要裁剪
    # 白色区域从约 16% 开始到 84% 结束，收紧以去除边缘脏像素
    left_crop = int(width * 0.16)
    right_crop = int(width * 0.84)

    # 裁剪图片
    cropped = img.crop((left_crop, top_crop, right_crop, bottom_crop))
    print(f"裁剪后尺寸: {cropped.size}")

    # 进一步压缩 - 调整大小到适合展示的尺寸
    # 保持宽高比，最大宽度 400 像素
    max_width = 400
    if cropped.width > max_width:
        ratio = max_width / cropped.width
        new_size = (max_width, int(cropped.height * ratio))
        cropped = cropped.resize(new_size, Image.Resampling.LANCZOS)
        print(f"缩放后尺寸: {cropped.size}")

    # 确保输出目录存在
    os.makedirs(os.path.dirname(output_path), exist_ok=True)

    # 保存为 PNG，使用优化压缩
    cropped.save(output_path, "PNG", optimize=True)

    # 获取文件大小
    file_size = os.path.getsize(output_path)
    print(f"输出文件大小: {file_size / 1024:.1f} KB")
    print(f"输出路径: {output_path}")


if __name__ == "__main__":
    process_qrcode()
