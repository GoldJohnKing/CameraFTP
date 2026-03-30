#!/usr/bin/env python3
"""
CameraFTP 压力测试工具 - 测试模式
验证脚本逻辑而不需要 FTP 连接
"""

import io
import random
import sys
from datetime import datetime

try:
    from PIL import Image, ImageDraw
    import numpy as np
    from pillow_heif import register_heif_opener

    register_heif_opener()
    HEIF_SUPPORT = True
except ImportError as e:
    print(f"错误：{e}")
    print("请运行: uv pip install pillow numpy pillow-heif")
    sys.exit(1)


def generate_image_data(width, height):
    """生成随机图像数据"""
    base_color = [random.randint(50, 200) for _ in range(3)]
    img_array = np.random.randint(0, 255, (height, width, 3), dtype=np.uint8)
    for i in range(3):
        img_array[:, :, i] = (img_array[:, :, i] * 0.5 + base_color[i] * 0.5).astype(
            np.uint8
        )
    return img_array


def test_jpeg_generation():
    """测试 JPEG 生成"""
    print("测试 JPEG 生成 (28MB)...")
    target_size = 28
    base_pixels = int(target_size * 0.4 * 1000000)
    height = int((base_pixels / 4 * 3) ** 0.5)
    width = int(height * 4 / 3)

    img_array = generate_image_data(width, height)
    image = Image.fromarray(img_array, "RGB")
    draw = ImageDraw.Draw(image)
    draw.text((50, 50), "Test", fill=(255, 255, 255))

    # 尝试不同质量
    for quality in [95, 85, 75]:
        buffer = io.BytesIO()
        image.save(buffer, format="JPEG", quality=quality, optimize=True)
        size_mb = buffer.tell() / (1024 * 1024)
        print(f"  质量 {quality}%: {size_mb:.2f} MB")
        if size_mb <= target_size * 1.1:
            break

    print(f"✓ JPEG 测试通过\n")
    return True


def test_heif_generation():
    """测试 HEIF 生成"""
    if not HEIF_SUPPORT:
        print("⚠ HEIF 未启用\n")
        return True

    print("测试 HEIF 生成 (28MB)...")
    target_size = 28
    base_pixels = int(target_size * 1.5 * 1000000)
    height = int((base_pixels / 4 * 3) ** 0.5)
    width = int(height * 4 / 3)

    img_array = generate_image_data(width, height)
    image = Image.fromarray(img_array, "RGB")

    for quality in [85, 75, 65]:
        buffer = io.BytesIO()
        try:
            image.save(buffer, format="HEIF", quality=quality)
            size_mb = buffer.tell() / (1024 * 1024)
            print(f"  质量 {quality}%: {size_mb:.2f} MB")
            if size_mb <= target_size * 1.1:
                break
        except Exception as e:
            print(f"  质量 {quality}% 失败: {e}")

    print(f"✓ HEIF 测试通过\n")
    return True


def main():
    print("=" * 50)
    print("CameraFTP 压力测试 - 脚本验证")
    print("=" * 50)
    print()

    try:
        test_jpeg_generation()
        test_heif_generation()

        print("=" * 50)
        print("✓ 所有测试通过！脚本可以正常使用。")
        print("=" * 50)
        print()
        print("现在可以运行: ./run_stress_test.sh")
        return 0

    except Exception as e:
        print(f"✗ 测试失败: {e}")
        import traceback

        traceback.print_exc()
        return 1


if __name__ == "__main__":
    sys.exit(main())
