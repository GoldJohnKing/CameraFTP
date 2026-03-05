#!/usr/bin/env python3
"""
处理捐赠二维码图片
- 移除顶部和底部的蓝绿色外框
- 保留中间二维码和下方白色背景的支付标识
- 裁剪微信支付和支付宝logo（使用边缘检测）
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

    # 底部裁剪到支付标识下方（白色区域底部约在 85% 位置）
    bottom_crop = int(height * 0.85)  # 约 85% 处，去除底部脏像素

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


def is_white_pixel(r, g, b, threshold=200):
    """判断像素是否为白色（或接近白色）
    降低阈值以更好地识别边缘
    """
    # 使用灰度值判断，更宽容
    gray = int(0.299 * r + 0.587 * g + 0.114 * b)
    return gray >= threshold


def is_background_color(r, g, b):
    """判断是否为蓝绿色背景"""
    # 绿色背景：G 值高，R 和 B 值低
    is_green = g > 150 and r < 100 and b < 100
    # 蓝色背景：B 值高，R 和 G 值低
    is_blue = b > 150 and r < 100 and g < 100
    return is_green or is_blue


def find_content_bounds(img, region_box, exclude_background=True):
    """
    在指定区域内查找内容的边界
    region_box: (left, top, right, bottom)
    exclude_background: 是否排除蓝绿色背景
    返回: (left, top, right, bottom) 包含内容的最小矩形
    """
    left, top, right, bottom = region_box

    pixels = img.load()

    # 初始化边界为区域边界
    content_left = right
    content_right = left
    content_top = bottom
    content_bottom = top

    # 扫描查找有效像素（非白色且非背景色）
    for y in range(top, bottom):
        for x in range(left, right):
            r, g, b = pixels[x, y][:3]

            # 跳过白色背景
            if is_white_pixel(r, g, b):
                continue

            # 如果需要，跳过蓝绿色背景
            if exclude_background and is_background_color(r, g, b):
                continue

            # 这是有效内容像素
            content_left = min(content_left, x)
            content_right = max(content_right, x)
            content_top = min(content_top, y)
            content_bottom = max(content_bottom, y)

    # 如果没有找到内容，返回原区域
    if content_left > content_right or content_top > content_bottom:
        return region_box

    return (content_left, content_top, content_right + 1, content_bottom + 1)


def process_payment_logos():
    """裁剪微信支付和支付宝logo（使用边缘检测）"""
    input_path = "donation/unified.png"

    # 打开图片
    img = Image.open(input_path)
    width, height = img.size

    # 定义大致区域：白色背景底部，包含两个logo的区域
    # 垂直方向约在 73%-82%（二维码下方的支付标识区域）
    rough_top = int(height * 0.73)
    rough_bottom = int(height * 0.82)

    # 微信区域：左侧，从约 18% 开始到 50% 结束
    # 避免左侧绿色背景，给右侧足够空间容纳完整文字（包括"Pay"）
    wechat_left = int(width * 0.18)
    wechat_right = int(width * 0.50)
    wechat_region = (wechat_left, rough_top, wechat_right, rough_bottom)

    # 支付宝区域：右侧，从约 52% 开始到 82% 结束
    # 确保在分割线右侧足够远，避免"微信支付"的文字
    alipay_left = int(width * 0.52)
    alipay_right = int(width * 0.82)
    alipay_region = (alipay_left, rough_top, alipay_right, rough_bottom)

    # 查找微信内容的精确边界
    wechat_bounds = find_content_bounds(img, wechat_region)
    print(f"微信原始边界: {wechat_bounds}")

    # 查找支付宝内容的精确边界
    alipay_bounds = find_content_bounds(img, alipay_region)
    print(f"支付宝原始边界: {alipay_bounds}")

    # 添加白边 padding（约 10 像素或比例的边距）
    padding = 15

    # 微信最终裁剪边界（添加padding）
    wechat_final = (
        max(0, wechat_bounds[0] - padding),
        max(0, wechat_bounds[1] - padding),
        min(width, wechat_bounds[2] + padding),
        min(height, wechat_bounds[3] + padding),
    )

    # 支付宝最终裁剪边界（添加padding）
    alipay_final = (
        max(0, alipay_bounds[0] - padding),
        max(0, alipay_bounds[1] - padding),
        min(width, alipay_bounds[2] + padding),
        min(height, alipay_bounds[3] + padding),
    )

    print(f"微信最终边界（含padding）: {wechat_final}")
    print(f"支付宝最终边界（含padding）: {alipay_final}")

    # 裁剪微信支付logo
    wechat_logo = img.crop(wechat_final)
    wechat_logo_path = "public/wechat-logo.png"
    wechat_logo.save(wechat_logo_path, "PNG", optimize=True)
    print(f"微信支付logo已保存: {wechat_logo_path}, 尺寸: {wechat_logo.size}")

    # 裁剪支付宝logo
    alipay_logo = img.crop(alipay_final)
    alipay_logo_path = "public/alipay-logo.png"
    alipay_logo.save(alipay_logo_path, "PNG", optimize=True)
    print(f"支付宝logo已保存: {alipay_logo_path}, 尺寸: {alipay_logo.size}")


if __name__ == "__main__":
    process_qrcode()
    process_payment_logos()
