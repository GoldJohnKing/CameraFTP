#!/usr/bin/env python3
"""
图标生成脚本 - 为 CameraFTP 生成应用图标

使用方法:
    cd src-tauri/icons && uv run python3 generate_icons.py

生成文件:
    - 128x128.png - 开始菜单/大图标
    - 32x32.png   - 任务栏图标
    - icon.ico    - 托盘区图标

图标样式:
    - 蓝色圆角矩形背景 (#2563EB, rounded-2xl 约22%圆角)
    - 白色 Lucide Camera SVG 图案居中
"""

import cairosvg
from PIL import Image, ImageDraw
import io

# Lucide Camera SVG (白色描边)
SVG_CONTENT = """<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M13.997 4a2 2 0 0 1 1.76 1.05l.486.9A2 2 0 0 0 18.003 7H20a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V9a2 2 0 0 1 2-2h1.997a2 2 0 0 0 1.759-1.048l.489-.904A2 2 0 0 1 10.004 4z" />
  <circle cx="12" cy="13" r="3" />
</svg>"""


def create_icon(size: int) -> Image.Image:
    """创建指定尺寸的图标"""
    # 渲染 SVG 为 PNG
    png_bytes = cairosvg.svg2png(
        bytestring=SVG_CONTENT, output_width=size, output_height=size
    )
    camera = Image.open(io.BytesIO(png_bytes)).convert("RGBA")

    # 将黑色相机转换为白色
    pixels = camera.load()
    for y in range(camera.height):
        for x in range(camera.width):
            r, g, b, a = pixels[x, y]
            if a > 0:
                pixels[x, y] = (255, 255, 255, a)

    # 创建蓝色圆角矩形背景
    bg = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(bg)

    blue = (0x25, 0x63, 0xEB, 255)  # blue-600
    radius = int(size * 0.22)  # rounded-2xl

    draw.rounded_rectangle([0, 0, size - 1, size - 1], radius=radius, fill=blue)

    # 缩放相机图案（保留边距）
    padding = int(size * 0.15)
    max_size = size - 2 * padding
    camera_resized = camera.resize((max_size, max_size), Image.Resampling.LANCZOS)

    # 居中粘贴
    x_offset = (size - max_size) // 2
    y_offset = (size - max_size) // 2
    result = bg.copy()
    result.paste(camera_resized, (x_offset, y_offset), camera_resized)

    return result


def main():
    print("Generating CameraFTP icons...")

    # 生成 128x128 主图标
    img128 = create_icon(128)
    img128.save("128x128.png", "PNG")
    print("✓ Created 128x128.png")

    # 生成 32x32（高质量缩放）
    img32 = img128.resize((32, 32), Image.Resampling.LANCZOS)
    img32.save("32x32.png", "PNG")
    print("✓ Created 32x32.png")

    # 生成 ICO 文件
    img32.save("icon.ico", format="ICO", sizes=[(32, 32)])
    print("✓ Created icon.ico")

    print("\nDone! Icons generated successfully.")


if __name__ == "__main__":
    main()
