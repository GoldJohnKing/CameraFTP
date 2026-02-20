"""
生成绿色版本的托盘图标（服务器运行时状态）
使用色彩滤镜：将蓝色转换为绿色
"""

from PIL import Image
import struct


def create_ico_from_image(img):
    """Create ICO from PIL Image"""
    rgba_data = list(img.getdata())
    size = img.width
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
            idx = y * size + x
            r, g, b, a = rgba_data[idx]
            ico += bytes([b, g, r, a])

    return ico


def blue_to_green_filter(img):
    """
    将图像中的蓝色转换为绿色
    使用像素级颜色替换，处理抗锯齿过渡
    """
    pixels = list(img.getdata())
    new_pixels = []

    for r, g, b, a in pixels:
        # 检测蓝色：蓝色分量占主导
        # 原蓝色: #2563EB (37, 99, 235)
        # 目标绿色: #22C55E (34, 197, 94)

        if b > 150 and b > r + 50 and b > g + 50:
            # 是蓝色区域，根据亮度转换为绿色
            # 计算亮度 (0-1)
            luminance = b / 255.0

            # 映射到绿色
            new_r = int(34 * luminance)
            new_g = int(197 * luminance)
            new_b = int(94 * luminance)

            new_pixels.append((new_r, new_g, new_b, a))
        else:
            # 非蓝色区域（白色描边、透明等）保持不变
            new_pixels.append((r, g, b, a))

    result = Image.new("RGBA", img.size)
    result.putdata(new_pixels)
    return result


def main():
    print("Generating green camera icons for tray...")

    # 读取蓝色图标
    blue_img = Image.open("32x32.png")
    blue_img = blue_img.convert("RGBA")

    # 应用蓝色到绿色的滤镜
    green_img = blue_to_green_filter(blue_img)

    # 保存为 PNG
    green_img.save("tray-active.png")
    print("Created tray-active.png (green - server running)")

    # 保存为 ICO
    ico = create_ico_from_image(green_img)
    with open("tray-active.ico", "wb") as f:
        f.write(ico)
    print("Created tray-active.ico (green - server running)")

    print("Done!")


if __name__ == "__main__":
    main()
