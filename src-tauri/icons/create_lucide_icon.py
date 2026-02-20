import struct
import zlib
import xml.etree.ElementTree as ET

# Lucide Camera SVG path data
SVG_PATHS = [
    # Camera body (the rounded rectangle with viewfinder)
    "M13.997 4a2 2 0 0 1 1.76 1.05l.486.9A2 2 0 0 0 18.003 7H20a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V9a2 2 0 0 1 2-2h1.997a2 2 0 0 0 1.759-1.048l.489-.904A2 2 0 0 1 10.004 4z",
    # Lens (circle)
    ("M12 13m-3 0a3 3 0 1 0 6 0a3 3 0 1 0 -6 0", "circle"),
]


def parse_path(d):
    """Simple SVG path parser for straight lines and curves"""
    import re

    # This is a simplified parser - for production use a proper SVG library
    # For now we'll use a rasterization approach
    return d


def rasterize_svg_path(d, width, height, stroke_width=2, color=(255, 255, 255, 255)):
    """Rasterize an SVG path to pixel array"""
    pixels = [0, 0, 0, 0] * (width * height)

    # Parse path commands
    import re

    tokens = re.findall(
        r"[MmLlHhVvCcQqTtSsAaZz]|[-+]?[0-9]*\.?[0-9]+(?:[eE][-+]?[0-9]+)?", d
    )

    # Simplified: draw based on known Camera path structure
    # The camera path has specific known points

    # For the camera body (complex rounded rect with viewfinder)
    # We know the key points from viewBox="0 0 24 24"

    # Draw filled shape
    def draw_line(x1, y1, x2, y2, width_px, color_val):
        """Draw a line on the pixel array"""
        dx = x2 - x1
        dy = y2 - y1
        steps = max(abs(dx), abs(dy)) * width_px
        if steps == 0:
            return
        for i in range(int(steps)):
            t = i / steps
            x = int(x1 + dx * t)
            y = int(y1 + dy * t)
            if 0 <= x < width and 0 <= y < height:
                for wy in range(-width_px // 2, width_px // 2 + 1):
                    for wx in range(-width_px // 2, width_px // 2 + 1):
                        px, py = x + wx, y + wy
                        if 0 <= px < width and 0 <= py < height:
                            idx = (py * width + px) * 4
                            pixels[idx : idx + 4] = list(color_val)

    def fill_rect(left, top, right, bottom, color_val):
        """Fill a rectangle"""
        for y in range(max(0, int(top)), min(height, int(bottom))):
            for x in range(max(0, int(left)), min(width, int(right))):
                idx = (y * width + x) * 4
                pixels[idx : idx + 4] = list(color_val)

    def draw_circle(cx, cy, r, color_val, fill=False):
        """Draw a circle"""
        r_int = int(r + 1)
        for y in range(max(0, int(cy) - r_int), min(height, int(cy) + r_int + 1)):
            for x in range(max(0, int(cx) - r_int), min(width, int(cx) + r_int + 1)):
                d = ((x - cx) ** 2 + (y - cy) ** 2) ** 0.5
                if fill and d <= r:
                    idx = (y * width + x) * 4
                    pixels[idx : idx + 4] = list(color_val)
                elif abs(d - r) <= 1:
                    idx = (y * width + x) * 4
                    pixels[idx : idx + 4] = list(color_val)

    # Scale from viewBox (24x24) to target size
    scale = width / 24
    sw = max(1, int(stroke_width * scale))

    # Camera icon geometry (based on lucide Camera SVG)
    # Body is a complex shape - approximated here
    margin = width * 0.18
    body_left = margin
    body_right = width - margin
    body_top = width * 0.28
    body_bottom = width * 0.82
    corner_r = width * 0.08

    # Draw camera body outline (thick stroke)
    def draw_rounded_rect_outline(
        left, top, right, bottom, radius, stroke_w, color_val
    ):
        """Draw rounded rectangle outline"""
        # Top and bottom edges
        draw_line(left + radius, top, right - radius, top, stroke_w, color_val)
        draw_line(left + radius, bottom, right - radius, bottom, stroke_w, color_val)
        # Left and right edges
        draw_line(left, top + radius, left, bottom - radius, stroke_w, color_val)
        draw_line(right, top + radius, right, bottom - radius, stroke_w, color_val)

    draw_rounded_rect_outline(
        body_left, body_top, body_right, body_bottom, corner_r, sw, color
    )

    # Viewfinder (top bump)
    vf_left = width * 0.38
    vf_right = width * 0.62
    vf_top = body_top - width * 0.12
    vf_bottom = body_top
    draw_line(vf_left, vf_bottom, vf_left, vf_top + corner_r, sw, color)
    draw_line(vf_right, vf_bottom, vf_right, vf_top + corner_r, sw, color)
    draw_line(vf_left + corner_r, vf_top, vf_right - corner_r, vf_top, sw, color)

    # Lens (circle in center)
    lens_cx = width * 0.5
    lens_cy = width * 0.54
    lens_r = width * 0.14
    draw_circle(lens_cx, lens_cy, lens_r, color, fill=False)

    return pixels


def create_png_rgba(width, height, pixels):
    """Create PNG from RGBA pixel data"""
    png = b"\x89PNG\r\n\x1a\n"

    ihdr_data = struct.pack("!IIBBBBB", width, height, 8, 6, 0, 0, 0)
    ihdr_crc = zlib.crc32(b"IHDR" + ihdr_data) & 0xFFFFFFFF
    png += (
        struct.pack("!I", len(ihdr_data))
        + b"IHDR"
        + ihdr_data
        + struct.pack("!I", ihdr_crc)
    )

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

    iend_crc = zlib.crc32(b"IEND") & 0xFFFFFFFF
    png += struct.pack("!I", 0) + b"IEND" + struct.pack("!I", iend_crc)

    return png


def draw_lucide_camera_icon(size):
    """Draw camera icon with blue background and white lucide-style camera"""
    # Blue background
    blue = (0x25, 0x63, 0xEB, 0xFF)
    white = (0xFF, 0xFF, 0xFF, 0xFF)

    # Start with blue background (rounded)
    pixels = list(blue) * (size * size)
    bg_radius = int(size * 0.22)

    # Make corners transparent
    for y in range(size):
        for x in range(size):
            idx = (y * size + x) * 4

            # Check if outside rounded rect
            in_corner = False
            corners = [
                (bg_radius, bg_radius),  # TL
                (size - bg_radius, bg_radius),  # TR
                (bg_radius, size - bg_radius),  # BL
                (size - bg_radius, size - bg_radius),  # BR
            ]
            for cx, cy in corners:
                if x < cx and y < cy:  # TL corner area
                    if (x - cx) ** 2 + (y - cy) ** 2 > bg_radius**2:
                        in_corner = True
                elif x >= cx and y < cy:  # TR
                    if (x - cx) ** 2 + (y - cy) ** 2 > bg_radius**2:
                        in_corner = True
                elif x < cx and y >= cy:  # BL
                    if (x - cx) ** 2 + (y - cy) ** 2 > bg_radius**2:
                        in_corner = True
                elif x >= cx and y >= cy:  # BR
                    if (x - cx) ** 2 + (y - cy) ** 2 > bg_radius**2:
                        in_corner = True

            if in_corner:
                pixels[idx : idx + 4] = [0, 0, 0, 0]

    # Draw white camera icon on top
    camera_pixels = rasterize_svg_path(
        SVG_PATHS[0], size, size, stroke_width=2, color=white
    )

    # Blend camera onto blue background
    for i in range(0, len(pixels), 4):
        if camera_pixels[i + 3] > 0:  # If camera pixel is not transparent
            pixels[i : i + 4] = camera_pixels[i : i + 4]

    return pixels


def create_ico_file(pixels_32):
    """Create ICO from 32x32 RGBA pixels"""
    size = 32
    ico = b""

    ico += struct.pack("<HHH", 0, 1, 1)

    bmp_size = 40 + (size * size * 4)
    ico += struct.pack("<BBBBHHII", size, size, 0, 0, 1, 32, bmp_size, 22)

    ico += struct.pack("<IiiHHIIiiII", 40, size, size * 2, 1, 32, 0, 0, 0, 0, 0, 0)

    for y in range(size - 1, -1, -1):
        for x in range(size):
            idx = (y * size + x) * 4
            r, g, b, a = pixels_32[idx : idx + 4]
            ico += bytes([b, g, r, a])

    return ico


# Generate
print("Generating lucide camera icons...")

for size in [32, 128]:
    pixels = draw_lucide_camera_icon(size)
    png = create_png_rgba(size, size, pixels)
    with open(f"{size}x{size}.png", "wb") as f:
        f.write(png)
    print(f"Created {size}x{size}.png")

pixels_32 = draw_lucide_camera_icon(32)
ico = create_ico_file(pixels_32)
with open("icon.ico", "wb") as f:
    f.write(ico)
print("Created icon.ico")

print("Done!")
