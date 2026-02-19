import struct
import zlib


# Create a simple valid PNG
def create_png_rgba(width, height, color):
    # PNG signature
    png = b"\x89PNG\r\n\x1a\n"

    # IHDR chunk
    ihdr_data = struct.pack("!IIBBBBB", width, height, 8, 6, 0, 0, 0)  # 8-bit RGBA
    ihdr_crc = zlib.crc32(b"IHDR" + ihdr_data) & 0xFFFFFFFF
    png += (
        struct.pack("!I", len(ihdr_data))
        + b"IHDR"
        + ihdr_data
        + struct.pack("!I", ihdr_crc)
    )

    # IDAT chunk - raw image data
    raw_data = b""
    for y in range(height):
        raw_data += b"\x00"  # Filter byte
        for x in range(width):
            raw_data += bytes(color)  # RGBA

    compressed = zlib.compress(raw_data)
    idat_crc = zlib.crc32(b"IDAT" + compressed) & 0xFFFFFFFF
    png += (
        struct.pack("!I", len(compressed))
        + b"IDAT"
        + compressed
        + struct.pack("!I", idat_crc)
    )

    # IEND chunk
    iend_crc = zlib.crc32(b"IEND") & 0xFFFFFFFF
    png += struct.pack("!I", 0) + b"IEND" + struct.pack("!I", iend_crc)

    return png


# Create simple icons
blue = [0x42, 0x87, 0xF5, 0xFF]  # Blue color

for size in [32, 128]:
    png = create_png_rgba(size, size, blue)
    with open(f"{size}x{size}.png", "wb") as f:
        f.write(png)
    print(f"Created {size}x{size}.png")

# Create a minimal valid ICO file (1x1 blue pixel)
# ICO format: https://en.wikipedia.org/wiki/ICO_(file_format)
ico = b""

# ICONDIR structure
ico += struct.pack("<HHH", 0, 1, 1)  # Reserved (0), Type (1=icon), Count (1)

# ICONDIRENTRY for 32x32
bmp_size = 32
bmp_data_size = 40 + (bmp_size * bmp_size * 4)  # BITMAPINFOHEADER + pixel data
ico += struct.pack(
    "<BBBBHHII",
    bmp_size,
    bmp_size,  # width, height
    0,
    0,  # color count, reserved
    1,
    32,  # planes, bit count
    bmp_data_size,  # size of image data
    22,  # offset to image data
)

# BITMAPINFOHEADER
ico += struct.pack(
    "<IiiHHIIiiII",
    40,  # biSize
    bmp_size,  # biWidth
    bmp_size * 2,  # biHeight (XOR + AND masks)
    1,  # biPlanes
    32,  # biBitCount (RGBA)
    0,  # biCompression
    0,  # biSizeImage
    0,
    0,  # biXPelsPerMeter, biYPelsPerMeter
    0,
    0,  # biClrUsed, biClrImportant
)

# Pixel data (BGRA format, bottom-up)
pixels = b""
for y in range(bmp_size):
    for x in range(bmp_size):
        # Blue square
        if 8 <= x < 24 and 8 <= y < 24:
            pixels += b"\xf5\x87\x42\xff"  # BGRA blue
        else:
            pixels += b"\x00\x00\x00\xff"  # BGRA black

ico += pixels

# No AND mask needed for 32-bit images

with open("icon.ico", "wb") as f:
    f.write(ico)
print("Created icon.ico")
