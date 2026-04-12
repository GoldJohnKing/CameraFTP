#!/usr/bin/env python3
"""
CameraFTP 压力测试工具
用于验证 FTP 服务器稳定性修复

功能：
- 生成 25-35MB 的真实 JPEG/HEIF 图片
- 随机间隔（0-15秒）上传
- 匿名登录 FTP 服务器
- 持续运行直到手动停止

用法：
    uv run ftp_stress_test.py
"""

import ftplib
import io
import random
import time
import sys
from datetime import datetime
from pathlib import Path

try:
    from PIL import Image, ImageDraw
    import numpy as np
except ImportError:
    print("错误：缺少必要的依赖包")
    print("请运行: uv pip install pillow numpy")
    sys.exit(1)

# 尝试导入 HEIF 支持
try:
    from pillow_heif import register_heif_opener

    register_heif_opener()
    HEIF_SUPPORT = True
    print("✓ HEIF 支持已启用")
except ImportError:
    HEIF_SUPPORT = False
    print("⚠ HEIF 支持未启用（pip install pillow-heif）")
HEIF_SUPPORT = False # 暂时移除HEIF支持

# 导入 EXIF 处理库
try:
    import piexif
    from piexif.helper import UserComment

    EXIF_SUPPORT = True
except ImportError:
    EXIF_SUPPORT = False
    print("⚠ EXIF 支持未启用（pip install piexif）")


# FTP 服务器配置
FTP_HOST = "192.168.1.214"
FTP_PORT = 2121
FTP_USER = "anonymous"
FTP_PASS = ""
UPLOAD_DIR = "/"  # 上传到根目录


def generate_image_data(width: int, height: int):
    """生成随机图像数据"""
    # 创建基础颜色
    base_color = [
        random.randint(50, 200),
        random.randint(50, 200),
        random.randint(50, 200),
    ]

    # 生成基础图像
    img_array = np.random.randint(0, 255, (height, width, 3), dtype=np.uint8)

    # 混合基础颜色
    for i in range(3):
        img_array[:, :, i] = (img_array[:, :, i] * 0.5 + base_color[i] * 0.5).astype(
            np.uint8
        )

    return img_array


def generate_exif_dict():
    """生成包含相机参数的 EXIF 数据"""
    if not EXIF_SUPPORT:
        return None

    # 随机相机参数
    iso = random.choice([100, 200, 400, 800, 1600, 3200, 6400])
    aperture = round(random.uniform(1.4, 22.0), 1)  # f/1.4 - f/22
    shutter_speed_denominator = random.choice([60, 125, 250, 500, 1000, 2000, 4000])
    focal_length = random.choice([24, 35, 50, 85, 105, 135, 200])

    # 快门速度表示为分数（如 1/125）
    # EXIF 中快门速度以 APEX 值存储，但这里我们存为字符串

    # 构建 EXIF 字典
    exif_dict = {
        "0th": {
            piexif.ImageIFD.Make: "CameraFTP".encode("utf-8"),
            piexif.ImageIFD.Model: "StressTest Camera".encode("utf-8"),
            piexif.ImageIFD.Software: "CameraFTP Test Tool".encode("utf-8"),
            piexif.ImageIFD.DateTime: datetime.now()
            .strftime("%Y:%m:%d %H:%M:%S")
            .encode("utf-8"),
        },
        "Exif": {
            piexif.ExifIFD.ISOSpeedRatings: iso,
            piexif.ExifIFD.FNumber: (int(aperture * 10), 10),  # 以有理数存储
            piexif.ExifIFD.ExposureTime: (1, shutter_speed_denominator),  # 快门速度 1/N
            piexif.ExifIFD.FocalLength: (focal_length, 1),
            piexif.ExifIFD.DateTimeOriginal: datetime.now()
            .strftime("%Y:%m:%d %H:%M:%S")
            .encode("utf-8"),
            piexif.ExifIFD.DateTimeDigitized: datetime.now()
            .strftime("%Y:%m:%d %H:%M:%S")
            .encode("utf-8"),
        },
    }

    return exif_dict, {
        "ISO": iso,
        "Aperture": f"f/{aperture}",
        "Shutter": f"1/{shutter_speed_denominator}s",
        "Focal": f"{focal_length}mm",
    }


def add_structured_content(image):
    """向图像添加结构化内容（几何图形等）"""
    draw = ImageDraw.Draw(image)
    width, height = image.size

    # 添加文字信息
    timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S.%f")[:-3]
    text = f"CameraFTP Test\n{timestamp}"

    # 尝试添加文字
    try:
        draw.text((50, 50), text, fill=(255, 255, 255))
    except Exception as e:
        # 如果没有字体，绘制矩形代替
        draw.rectangle(
            [50, 50, 400, 150], fill=(100, 100, 100), outline=(255, 255, 255), width=2
        )

    # 添加一些几何图形
    for _ in range(random.randint(5, 15)):
        x1 = random.randint(0, width - 100)
        y1 = random.randint(0, height - 100)
        x2 = x1 + random.randint(50, 200)
        y2 = y1 + random.randint(50, 200)
        color = tuple(random.randint(0, 255) for _ in range(3))
        draw.rectangle(
            [x1, y1, min(x2, width - 1), min(y2, height - 1)], outline=color, width=2
        )


def save_jpeg_with_size(image, target_size_mb):
    """保存为 JPEG 并调整大小，添加 EXIF 信息"""
    best_data = None
    best_diff = float("inf")
    exif_bytes = None

    # 生成 EXIF 数据
    exif_dict, exif_info = None, None
    if EXIF_SUPPORT:
        exif_dict, exif_info = generate_exif_dict()
        if exif_dict:
            exif_bytes = piexif.dump(exif_dict)
            print(
                f"    EXIF: ISO{exif_info['ISO']}, {exif_info['Aperture']}, {exif_info['Shutter']}, {exif_info['Focal']}"
            )

    for quality in range(95, 60, -5):
        buffer = io.BytesIO()
        # 保存时附加 EXIF
        save_kwargs = {"format": "JPEG", "quality": quality, "optimize": True}
        if exif_bytes:
            save_kwargs["exif"] = exif_bytes

        image.save(buffer, **save_kwargs)
        size_mb = buffer.tell() / (1024 * 1024)
        diff = abs(size_mb - target_size_mb)

        print(f"    JPEG 质量 {quality}% -> {size_mb:.2f}MB")

        if diff < best_diff:
            best_diff = diff
            best_data = buffer.getvalue()

        if size_mb <= target_size_mb * 1.05:  # 允许 5% 误差
            break

    return best_data


def save_heif_with_size(image, target_size_mb):
    """保存为 HEIF 并调整大小，添加 EXIF 信息"""
    if not HEIF_SUPPORT:
        raise RuntimeError("HEIF 支持未启用")

    best_data = None
    best_diff = float("inf")
    exif_bytes = None

    # 生成 EXIF 数据
    exif_dict, exif_info = None, None
    if EXIF_SUPPORT:
        exif_dict, exif_info = generate_exif_dict()
        if exif_dict:
            exif_bytes = piexif.dump(exif_dict)
            print(
                f"    EXIF: ISO{exif_info['ISO']}, {exif_info['Aperture']}, {exif_info['Shutter']}, {exif_info['Focal']}"
            )

    # HEIF 使用 quality 参数 (0-100)
    for quality in range(85, 40, -10):
        buffer = io.BytesIO()
        try:
            # HEIF 也支持 exif 参数
            save_kwargs = {"format": "HEIF", "quality": quality}
            if exif_bytes:
                save_kwargs["exif"] = exif_bytes

            image.save(buffer, **save_kwargs)
            size_mb = buffer.tell() / (1024 * 1024)
            diff = abs(size_mb - target_size_mb)

            print(f"    HEIF 质量 {quality}% -> {size_mb:.2f}MB")

            if diff < best_diff:
                best_diff = diff
                best_data = buffer.getvalue()

            if size_mb <= target_size_mb * 1.05:
                break
        except Exception as e:
            print(f"    HEIF 质量 {quality}% 失败: {e}")
            continue

    if best_data is None:
        raise RuntimeError("无法生成 HEIF 图像")

    return best_data


def generate_image(target_size_mb, fmt="JPEG"):
    """
    生成指定大小和格式的图像

    Returns:
        (image_data, extension)
    """
    # 估算分辨率
    # HEIF 压缩率比 JPEG 高约 2-3 倍，需要更大分辨率
    if fmt.upper() == "HEIF":
        base_pixels = int(target_size_mb * 1.5 * 1000000)  # HEIF 需要更多像素
    else:
        base_pixels = int(target_size_mb * 0.4 * 1000000)  # JPEG

    # 计算分辨率 (4:3 比例)
    height = int((base_pixels / 4 * 3) ** 0.5)
    width = int(height * 4 / 3)

    # 添加随机变化
    width = random.randint(width - 200, width + 200)
    height = random.randint(height - 150, height + 150)
    width = max(width, 1000)
    height = max(height, 1000)

    print(f"  生成 {width}x{height} 图像 ({fmt} 格式)...")

    # 生成图像数据
    img_array = generate_image_data(width, height)
    image = Image.fromarray(img_array, "RGB")

    # 添加结构化内容
    add_structured_content(image)

    # 保存为指定格式
    print(f"  压缩为 {fmt} (目标 {target_size_mb}MB)...")

    if fmt.upper() == "JPEG":
        data = save_jpeg_with_size(image, target_size_mb)
        ext = ".jpg"
    elif fmt.upper() == "HEIF":
        data = save_heif_with_size(image, target_size_mb)
        ext = ".heic"
    else:
        raise ValueError(f"不支持的格式: {fmt}")

    actual_size_mb = len(data) / (1024 * 1024)
    print(f"  实际大小: {actual_size_mb:.2f} MB")

    return data, ext


def upload_to_ftp(data, filename):
    """上传数据到 FTP 服务器"""
    try:
        print(f"  连接到 {FTP_HOST}:{FTP_PORT}...")

        ftp = ftplib.FTP()
        ftp.connect(FTP_HOST, FTP_PORT, timeout=30)
        ftp.login(FTP_USER, FTP_PASS)

        print(f"  登录成功，开始上传 {filename}...")

        if UPLOAD_DIR != "/":
            ftp.cwd(UPLOAD_DIR)

        buffer = io.BytesIO(data)
        start_time = time.time()

        ftp.storbinary(f"STOR {filename}", buffer)

        upload_time = time.time() - start_time
        size_mb = len(data) / (1024 * 1024)
        speed = size_mb / upload_time if upload_time > 0 else 0

        print(f"  上传完成: {upload_time:.2f}s ({speed:.2f} MB/s)")

        ftp.quit()
        return True

    except Exception as e:
        print(f"  上传失败: {e}")
        return False


def main():
    """主测试循环"""
    print("=" * 60)
    print("CameraFTP 压力测试工具")
    print("=" * 60)
    print(f"目标服务器: {FTP_HOST}:{FTP_PORT}")
    print(f"上传目录: {UPLOAD_DIR}")
    print(f"图片大小: 25-35 MB")
    print(f"图片格式: JPEG / {'HEIF' if HEIF_SUPPORT else 'JPEG only (HEIF未启用) '}")
    print(f"上传间隔: 0-15 秒随机")
    print("=" * 60)
    print()

    stats = {
        "total_uploads": 0,
        "successful_uploads": 0,
        "failed_uploads": 0,
        "total_mb": 0,
        "jpeg_count": 0,
        "heif_count": 0,
        "start_time": time.time(),
    }

    try:
        while True:
            target_size = random.uniform(25, 35)
            delay = random.uniform(0, 15)

            # 随机选择格式
            if HEIF_SUPPORT and random.random() < 0.5:
                fmt = "HEIF"
                stats["heif_count"] += 1
            else:
                fmt = "JPEG"
                stats["jpeg_count"] += 1

            timestamp = datetime.now().strftime("%Y%m%d_%H%M%S_%f")[:-3]
            filename = f"test_{timestamp}_{target_size:.1f}MB{'' if fmt == 'JPEG' else '_HEIF'}{'.jpg' if fmt == 'JPEG' else '.heic'}"

            print(
                f"\n[{datetime.now().strftime('%H:%M:%S')}] 准备上传 #{stats['total_uploads'] + 1}"
            )
            print(f"  文件名: {filename}")
            print(f"  格式: {fmt}")
            print(f"  目标大小: {target_size:.1f} MB")
            print(f"  等待间隔: {delay:.1f} 秒")
            print()

            if delay > 0:
                time.sleep(delay)

            # 生成图片
            print(f"[{datetime.now().strftime('%H:%M:%S')}] 生成图片...")
            try:
                image_data, ext = generate_image(target_size, fmt)
            except Exception as e:
                print(f"  生成图片失败: {e}")
                stats["failed_uploads"] += 1
                stats["total_uploads"] += 1
                continue

            # 上传
            print(f"[{datetime.now().strftime('%H:%M:%S')}] 开始上传...")
            if upload_to_ftp(image_data, filename):
                stats["successful_uploads"] += 1
                stats["total_mb"] += len(image_data) / (1024 * 1024)
                print(f"  上传成功")
            else:
                stats["failed_uploads"] += 1
                print(f"  上传失败")

            stats["total_uploads"] += 1

            # 显示统计
            elapsed = time.time() - stats["start_time"]
            avg_speed = stats["total_mb"] / elapsed if elapsed > 0 else 0
            success_rate = (
                (stats["successful_uploads"] / stats["total_uploads"] * 100)
                if stats["total_uploads"] > 0
                else 0
            )

            print(f"\n  --- 统计 ---")
            print(
                f"  总上传: {stats['total_uploads']} | 成功: {stats['successful_uploads']} | 失败: {stats['failed_uploads']}"
            )
            print(f"  JPEG: {stats['jpeg_count']} | HEIF: {stats['heif_count']}")
            print(f"  成功率: {success_rate:.1f}% | 总流量: {stats['total_mb']:.1f} MB")
            print(f"  平均速度: {avg_speed:.2f} MB/s | 运行时间: {elapsed:.0f}s")
            print(f"  {'=' * 50}")

    except KeyboardInterrupt:
        print("\n\n用户中断测试")

    # 最终统计
    print("\n" + "=" * 60)
    print("测试完成")
    print("=" * 60)
    elapsed = time.time() - stats["start_time"]
    print(f"总上传次数: {stats['total_uploads']}")
    print(f"成功: {stats['successful_uploads']} | 失败: {stats['failed_uploads']}")
    print(f"JPEG: {stats['jpeg_count']} | HEIF: {stats['heif_count']}")
    print(
        f"成功率: {(stats['successful_uploads'] / stats['total_uploads'] * 100):.1f}%"
    )
    print(f"总流量: {stats['total_mb']:.1f} MB")
    print(f"运行时间: {elapsed:.0f} 秒")
    print(f"平均速度: {stats['total_mb'] / elapsed:.2f} MB/s")


if __name__ == "__main__":
    main()
