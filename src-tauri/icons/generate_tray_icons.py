#!/usr/bin/env python3
"""
托盘图标生成脚本 - 生成带状态指示圆点的托盘图标

使用方法:
    cd src-tauri/icons && uv run python3 generate_tray_icons.py

生成文件:
    - tray-stopped.png      - 蓝色底色 + 红色圆点（服务器未启动）
    - tray-idle.png         - 蓝色底色 + 黄色圆点（服务器运行但无连接）
    - tray-active.png       - 蓝色底色 + 绿色圆点（服务器运行且有连接）

图标样式:
    - 蓝色圆角矩形背景 (#2563EB)
    - 白色 Lucide Camera SVG 图案居中
    - 右上角彩色实心圆点表示状态
"""

import cairosvg
from PIL import Image, ImageDraw
import io

# Lucide Camera SVG (白色描边)
SVG_CONTENT = """<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
  <path d="M13.997 4a2 2 0 0 1 1.76 1.05l.486.9A2 2 0 0 0 18.003 7H20a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V9a2 2 0 0 1 2-2h1.997a2 2 0 0 0 1.759-1.048l.489-.904A2 2 0 0 1 10.004 4z" />
  <circle cx="12" cy="13" r="3" />
</svg>"""

# 状态颜色定义
STATUS_COLORS = {
    "stopped": (0xEF, 0x44, 0x44),  # red-500
    "idle": (0xEA, 0xB3, 0x08),  # yellow-500
    "active": (0x22, 0xC5, 0x5E),  # green-500
}


def create_base_icon(size: int) -> Image.Image:
    """创建蓝色底色的基础图标（不含状态圆点）"""
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


def add_status_dot(icon: Image.Image, color: tuple, size: int = 32) -> Image.Image:
    """在图标右上角添加状态指示圆点（可越出蓝色矩形，但不越出画布边界）"""
    result = icon.copy()
    draw = ImageDraw.Draw(result)

    # 计算圆点大小（统一使用 35% 比例）
    dot_ratio = 0.35
    dot_size = max(6, int(size * dot_ratio))

    # 圆点位置：右上角，右侧贴右边界（不越界），上方贴上边界
    # 画布坐标范围是 0 到 size-1，圆点右边缘应在 size-1
    center_x = size - 1 - dot_size // 2
    center_y = dot_size // 2

    # 绘制圆点
    dot_color = (*color, 255)
    draw.ellipse(
        [
            center_x - dot_size // 2,
            center_y - dot_size // 2,
            center_x + dot_size // 2,
            center_y + dot_size // 2,
        ],
        fill=dot_color,
    )

    return result


def generate_tray_icon(state: str, size: int = 32) -> Image.Image:
    """生成指定状态的托盘图标"""
    base = create_base_icon(size)
    color = STATUS_COLORS[state]
    return add_status_dot(base, color, size)


def main():
    print("Generating Camera FTP Companion tray icons with status dots...")

    # 生成 32x32 托盘图标
    icon_size = 32

    # 生成三种状态的图标
    states = [
        ("stopped", "red dot - server stopped"),
        ("idle", "yellow dot - server running, no clients"),
        ("active", "green dot - server running, clients connected"),
    ]

    for state, description in states:
        icon = generate_tray_icon(state, icon_size)
        filename = f"tray-{state}.png"
        icon.save(filename, "PNG")
        print(f"  Created {filename} ({description})")

    # 同时生成 128x128 版本用于高 DPI 显示
    print("\nGenerating 128x128 versions for high DPI...")
    for state, _ in states:
        icon = generate_tray_icon(state, 128)
        filename = f"tray-{state}@4x.png"
        icon.save(filename, "PNG")
        print(f"  Created {filename}")

    print("\nDone! All tray icons generated successfully.")
    print("\nUsage in Rust:")
    print("  - tray-stopped.png: Server not running (red dot)")
    print("  - tray-idle.png: Server running, no connections (yellow dot)")
    print("  - tray-active.png: Server running, has connections (green dot)")


if __name__ == "__main__":
    main()
