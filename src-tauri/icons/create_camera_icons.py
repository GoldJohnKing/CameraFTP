import struct
import zlib
import math


def create_png_rgba(width, height, pixels):
    """Create PNG from RGBA pixel data"""
    png = b"\x89PNG\r\n\x1a\n"

    # IHDR chunk
    ihdr_data = struct.pack("!IIBBBBB", width, height, 8, 6, 0, 0, 0)
    ihdr_crc = zlib.crc32(b"IHDR" + ihdr_data) & 0xFFFFFFFF
    png += (
        struct.pack("!I", len(ihdr_data))
        + b"IHDR"
        + ihdr_data
        + struct.pack("!I", ihdr_crc)
    )

    # IDAT chunk
    raw_data = b""
    for y in range(height):
        raw_data += b"\x00"
        for x in range(width):
            idx = (y * width + x) * 4
            raw_data += bytes(pixels[idx : idx + 4])

    compressed = zlib.compress(raw_data)
    idat_crc = zlib.crc32(b"IDAT" + compressed) & 0xFFFFFFFF
    png += (
        struct.pack("!I", len(compressed))
        + b"IDAT"
        + compressed
        + struct.pack("!I", idat_crc)
    )

    # IEND
    iend_crc = zlib.crc32(b"IEND") & 0xFFFFFFFF
    png += struct.pack("!I", 0) + b"IEND" + struct.pack("!I", iend_crc)

    return png


def draw_lucide_camera(size):
    """
    精准复刻 lucide-react Camera 图标
    特点：
    - 蓝色方形圆角背景 (rounded-2xl 样式，约20%圆角)
    - 白色相机轮廓，粗描边
    - 圆角矩形机身
    - 中间同心圆镜头
    - 顶部中央取景器（小圆角矩形）
    """
    pixels = [0x00, 0x00, 0x00, 0x00] * (size * size)

    # 颜色
    blue = (0x25, 0x63, 0xEB, 0xFF)  # blue-600 #2563EB
    white = (0xFF, 0xFF, 0xFF, 0xFF)  # 纯白

    center = size // 2

    # 背景圆角 (类似 rounded-2xl)
    bg_radius = int(size * 0.22)

    # 相机参数 - 与 App.tsx 中 Camera 图标类似
    padding = int(size * 0.18)  # 内边距
    body_left = padding
    body_right = size - padding
    body_top = int(size * 0.28)  # 偏下给取景器留空间
    body_bottom = size - padding + int(size * 0.05)
    body_radius = int(size * 0.12)  # 机身圆角

    stroke = max(2, int(size * 0.09))  # 描边粗细

    def is_in_rounded_rect(x, y, left, top, right, bottom, radius):
        """检查点是否在圆角矩形内"""
        # 主体
        if left + radius <= x < right - radius and top <= y < bottom:
            return True
        if left <= x < right and top + radius <= y < bottom - radius:
            return True
        # 圆角
        corners = [
            (left + radius, top + radius),
            (right - radius, top + radius),
            (left + radius, bottom - radius),
            (right - radius, bottom - radius),
        ]
        for cx, cy in corners:
            if (x - cx) ** 2 + (y - cy) ** 2 < radius**2:
                return True
        return False

    def dist_to_rounded_rect_border(x, y, left, top, right, bottom, radius):
        """计算点到圆角矩形边框的带符号距离"""
        inside = is_in_rounded_rect(x, y, left, top, right, bottom, radius)

        # 简化的距离计算
        if left + radius <= x <= right - radius:
            if top + radius <= y <= bottom - radius:
                # 矩形内部
                d_left = x - left
                d_right = right - x
                d_top = y - top
                d_bottom = bottom - y
                min_d = min(d_left, d_right, d_top, d_bottom)
                return -min_d if inside else min_d

        # 简单近似
        cx = max(left + radius, min(x, right - radius))
        cy = max(top + radius, min(y, bottom - radius))
        dist = math.sqrt((x - cx) ** 2 + (y - cy) ** 2)

        if inside:
            return -dist
        else:
            return dist

    for y in range(size):
        for x in range(size):
            idx = (y * size + x) * 4

            # 1. 蓝色背景（圆角）
            if is_in_rounded_rect(x, y, 0, 0, size, size, bg_radius):
                pixels[idx : idx + 4] = list(blue)
            else:
                continue

            # 2. 相机机身轮廓（白色描边）
            dist = dist_to_rounded_rect_border(
                x, y, body_left, body_top, body_right, body_bottom, body_radius
            )
            if abs(dist) <= stroke / 2:
                pixels[idx : idx + 4] = list(white)
            elif dist < 0:
                # 机身内部保持蓝色背景
                pass

            # 3. 取景器（顶部小矩形，居中）
            vf_width = int((body_right - body_left) * 0.35)
            vf_height = int(size * 0.10)
            vf_left = center - vf_width // 2
            vf_top = body_top - vf_height - int(stroke * 0.5)
            vf_bottom = body_top - int(stroke * 0.5)
            vf_radius = int(size * 0.06)

            if is_in_rounded_rect(
                x, y, vf_left, vf_top, vf_left + vf_width, vf_bottom, vf_radius
            ):
                pixels[idx : idx + 4] = list(white)

            # 4. 镜头 - 外圈（白色圆环）
            lens_r = int(size * 0.16)
            lens_cx = center
            lens_cy = center + int(size * 0.02)
            dist_to_lens = math.sqrt((x - lens_cx) ** 2 + (y - lens_cy) ** 2)

            if abs(dist_to_lens - lens_r) <= stroke / 2:
                pixels[idx : idx + 4] = list(white)

            # 5. 镜头中心（白色填充圆）
            inner_r = int(lens_r * 0.45)
            if dist_to_lens <= inner_r:
                pixels[idx : idx + 4] = list(white)

    return pixels


def create_ico_file(pixels_32):
    """Create ICO from 32x32 RGBA pixels"""
    size = 32
    ico = b""

    # ICONDIR
    ico += struct.pack("<HHH", 0, 1, 1)

    # ICONDIRENTRY
    bmp_size = 40 + (size * size * 4)
    ico += struct.pack("<BBBBHHII", size, size, 0, 0, 1, 32, bmp_size, 22)

    # BITMAPINFOHEADER
    ico += struct.pack("<IiiHHIIiiII", 40, size, size * 2, 1, 32, 0, 0, 0, 0, 0, 0)

    # BGRA data (bottom-up)
    for y in range(size - 1, -1, -1):
        for x in range(size):
            idx = (y * size + x) * 4
            r, g, b, a = pixels_32[idx : idx + 4]
            ico += bytes([b, g, r, a])

    return ico


# Generate icons
print("Generating lucide-style camera icons...")

for size in [32, 128]:
    pixels = draw_lucide_camera(size)
    png = create_png_rgba(size, size, pixels)
    with open(f"{size}x{size}.png", "wb") as f:
        f.write(png)
    print(f"Created {size}x{size}.png")

# ICO
pixels_32 = draw_lucide_camera(32)
ico = create_ico_file(pixels_32)
with open("icon.ico", "wb") as f:
    f.write(ico)
print("Created icon.ico")

print("Done!")
