#!/usr/bin/env python3
"""RAW → Ultra HDR feasibility validation.

Extracts embedded JPEG from RAW, decodes RAW for HDR data,
computes gain map, and assembles Ultra HDR JPEG.
"""

import argparse
import io
import struct
import sys

import numpy as np
import rawpy
from PIL import Image
from scipy.ndimage import zoom as ndimage_zoom

# ── Constants ──────────────────────────────────────────────────────────────────

XMP_NAMESPACE = b"http://ns.adobe.com/xap/1.0/\x00"
ISO_NAMESPACE = b"urn:iso:std:iso:ts:21496:-1\x00"

# BT.709 luminance weights
LUM_R, LUM_G, LUM_B = 0.2126, 0.7152, 0.0722

EPSILON_SDR = 1.0 / 64.0
EPSILON_HDR = 1.0 / 64.0


# ── Step 1: Extract embedded JPEG ─────────────────────────────────────────────


def extract_embedded_jpeg(raw_path: str) -> bytes:
    """Extract the largest embedded JPEG preview from RAW file."""
    with rawpy.imread(raw_path) as raw:
        thumb = raw.extract_thumb()
        if thumb.format == rawpy.ThumbFormat.JPEG:
            return thumb.data
        raise ValueError(
            f"Thumbnail format is {thumb.format}, not JPEG. "
            "This RAW file may not contain an embedded JPEG preview."
        )


# ── Step 2: Decode RAW to linear HDR ──────────────────────────────────────────


def decode_raw_linear(raw_path: str, half_size: bool = False) -> np.ndarray:
    """Decode RAW to linear 16-bit sRGB (gamma=1, no tone curve)."""
    with rawpy.imread(raw_path) as raw:
        return raw.postprocess(
            output_bps=16,
            gamma=(1, 1),
            no_auto_bright=True,
            use_camera_wb=True,
            output_color=rawpy.ColorSpace.sRGB,
            half_size=half_size,
        )


# ── Step 3: Gain map computation (direct comparison + GainMapMin=0) ────────────


def srgb_to_linear(x: np.ndarray) -> np.ndarray:
    """sRGB gamma → linear (piecewise)."""
    return np.where(x <= 0.04045, x / 12.92, ((x + 0.055) / 1.055) ** 2.4)


def _downsample_avg(arr: np.ndarray, target_h: int, target_w: int) -> np.ndarray:
    """Downsample 2D array by block-averaging (anti-alias).

    Center-crops when dimensions don't divide evenly, so edge content
    is preserved symmetrically rather than trimmed from bottom-right.
    """
    h, w = arr.shape[:2]
    trim_h = (h // target_h) * target_h
    trim_w = (w // target_w) * target_w
    top = (h - trim_h) // 2
    left = (w - trim_w) // 2
    trimmed = arr[top : top + trim_h, left : left + trim_w]
    if arr.ndim == 3:
        return trimmed.reshape(
            target_h, trim_h // target_h, target_w, trim_w // target_w, arr.shape[2]
        ).mean(axis=(1, 3))
    return trimmed.reshape(
        target_h, trim_h // target_h, target_w, trim_w // target_w
    ).mean(axis=(1, 3))


def _center_crop(arr: np.ndarray, target_h: int, target_w: int) -> np.ndarray:
    """Center-crop 2D array to target dimensions."""
    h, w = arr.shape[:2]
    top = (h - target_h) // 2
    left = (w - target_w) // 2
    return arr[top : top + target_h, left : left + target_w]


def compute_gainmap(
    hdr: np.ndarray,
    sdr: np.ndarray,
    scale: int = 4,
) -> tuple[np.ndarray, dict]:
    """Compute gain map via direct comparison with GainMapMin=0.

    Directly compares linear HDR luminance with linear SDR luminance.
    Negative gains (midtones where camera boosted) are clamped to 0.
    Positive gains (highlights where camera compressed) are preserved.

    Returns (gainmap_8bit, metadata_dict).
    """
    sdr_h, sdr_w = sdr.shape[:2]
    hdr_h, hdr_w = hdr.shape[:2]

    # ── Align HDR to SDR ──────────────────────────────────────────────────
    hdr_f = hdr.astype(np.float64) / 65535.0
    if hdr_h != sdr_h or hdr_w != sdr_w:
        aspect_match = abs(hdr_w / hdr_h - sdr_w / sdr_h) / (sdr_w / sdr_h) < 0.01
        if aspect_match and hdr_h > sdr_h and hdr_w > sdr_w:
            hdr_f = _center_crop(hdr_f, sdr_h, sdr_w)
        else:
            from scipy.ndimage import zoom as ndimage_zoom
            hdr_f = ndimage_zoom(
                hdr_f, (sdr_h / hdr_h, sdr_w / hdr_w, 1), order=1
            )

    # ── Compute luminance at native resolution ─────────────────────────────
    Y_hdr = LUM_R * hdr_f[:, :, 0] + LUM_G * hdr_f[:, :, 1] + LUM_B * hdr_f[:, :, 2]

    sdr_lin = srgb_to_linear(sdr.astype(np.float64) / 255.0)
    Y_sdr = (
        LUM_R * sdr_lin[:, :, 0] + LUM_G * sdr_lin[:, :, 1] + LUM_B * sdr_lin[:, :, 2]
    )

    # ── Downsample both to gain map resolution ─────────────────────────────
    target_gm_h = sdr_h // scale
    target_gm_w = sdr_w // scale
    Y_hdr_down = _downsample_avg(Y_hdr, target_gm_h, target_gm_w)
    Y_sdr_down = _downsample_avg(Y_sdr, target_gm_h, target_gm_w)

    # ── Direct comparison ──────────────────────────────────────────────────
    gain = (Y_hdr_down + EPSILON_HDR) / (Y_sdr_down + EPSILON_SDR)
    log_gain = np.log2(np.clip(gain, 1e-10, None))

    # GainMapMin = 0: clamp negative gains, only encode brightening
    max_log = float(max(np.percentile(log_gain, 99), 0.01))
    recovery = np.clip(log_gain / max_log, 0.0, 1.0)
    gainmap = np.round(recovery * 255).astype(np.uint8)

    metadata = {
        "GainMapMin": 0.0,
        "GainMapMax": max_log,
        "Gamma": 1.0,
        "OffsetSDR": EPSILON_SDR,
        "OffsetHDR": EPSILON_HDR,
        "HDRCapacityMin": 0.0,
        "HDRCapacityMax": max_log,
        "BaseRenditionIsHDR": False,
    }
    return gainmap, metadata


# ── Step 4: Encode gain map as JPEG ───────────────────────────────────────────


def encode_gainmap_jpeg(gainmap: np.ndarray, quality: int = 75) -> bytes:
    """Encode 8-bit grayscale gain map to JPEG bytes."""
    buf = io.BytesIO()
    Image.fromarray(gainmap, mode="L").save(buf, format="JPEG", quality=quality)
    return buf.getvalue()


# ── Step 5: JPEG marker utilities ─────────────────────────────────────────────


def find_exif_in_jpeg(jpeg: bytes) -> bytes | None:
    """Extract raw EXIF APP1 data (length + Exif\0\0 + TIFF) from JPEG.

    Returns None if no EXIF found. The returned bytes do NOT include
    the FF E1 marker — only the length field onward.
    """
    pos = 2  # skip SOI
    while pos < len(jpeg) - 4:
        if jpeg[pos] != 0xFF:
            break
        pos += 1
        while pos < len(jpeg) and jpeg[pos] == 0xFF:
            pos += 1
        marker = jpeg[pos]
        pos += 1
        if marker in (0xDA, 0xD9):
            break
        if 0xD0 <= marker <= 0xD7:
            continue
        length = struct.unpack(">H", jpeg[pos : pos + 2])[0]
        if marker == 0xE1:
            data = jpeg[pos + 2 : pos + length]
            if data[:4] == b"Exif":
                return jpeg[pos : pos + length]
        pos += length
    return None


def strip_app1_markers(jpeg: bytes) -> bytearray:
    """Strip all APP1 (EXIF + XMP) markers from JPEG, keep everything else.

    Returns JPEG data starting right after SOI (SOI excluded).
    """
    out = bytearray()
    pos = 2  # skip SOI
    while pos < len(jpeg) - 4:
        if jpeg[pos] != 0xFF:
            # Entropy data — copy rest and break
            out.extend(jpeg[pos:])
            break
        marker_pos = pos
        pos += 1
        while pos < len(jpeg) and jpeg[pos] == 0xFF:
            pos += 1
        marker = jpeg[pos]
        pos += 1

        if marker in (0xD8, 0xD9):
            out.extend(jpeg[marker_pos : pos - 1])
            continue
        if marker == 0xDA:
            # SOS — copy marker header + all remaining entropy data
            length = struct.unpack(">H", jpeg[pos : pos + 2])[0]
            out.extend(jpeg[marker_pos : pos + length])
            entropy_start = pos + length
            out.extend(jpeg[entropy_start:])
            break
        if 0xD0 <= marker <= 0xD7:
            out.extend(jpeg[marker_pos:pos])
            continue

        length = struct.unpack(">H", jpeg[pos : pos + 2])[0]
        if marker != 0xE1:
            out.extend(jpeg[marker_pos : pos + length])
        pos += length
    return out


# ── Step 6: Ultra HDR container assembly ──────────────────────────────────────


def build_primary_xmp(gainmap_jpeg_size: int) -> bytes:
    """XMP for the primary image: GContainer directory + hdrgm:Version."""
    xmp = (
        '<?xpacket begin="\xef\xbb\xbf" id="W5M0MpCehiHzreSzNTczkc9d"?>\n'
        '<x:xmpmeta xmlns:x="adobe:ns:meta/">\n'
        ' <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">\n'
        '  <rdf:Description rdf:about=""\n'
        '    xmlns:Container="http://ns.google.com/photos/1.0/container/"\n'
        '    xmlns:Item="http://ns.google.com/photos/1.0/container/item/"\n'
        '    xmlns:hdrgm="http://ns.adobe.com/hdr-gain-map/1.0/"\n'
        f'    hdrgm:Version="1.0">\n'
        "   <Container:Directory>\n"
        "    <rdf:Seq>\n"
        '     <rdf:li rdf:parseType="Resource">\n'
        '      <Container:Item Item:Semantic="Primary" Item:Mime="image/jpeg"/>\n'
        "     </rdf:li>\n"
        '     <rdf:li rdf:parseType="Resource">\n'
        '      <Container:Item Item:Semantic="GainMap" Item:Mime="image/jpeg"\n'
        f'       Item:Length="{gainmap_jpeg_size}"/>\n'
        "     </rdf:li>\n"
        "    </rdf:Seq>\n"
        "   </Container:Directory>\n"
        "  </rdf:Description>\n"
        " </rdf:RDF>\n"
        "</x:xmpmeta>\n"
        '<?xpacket end="w"?>'
    )
    return xmp.encode("utf-8")


def build_gainmap_xmp(metadata: dict) -> bytes:
    """XMP for the gain map image: full hdrgm metadata."""
    xmp = (
        '<?xpacket begin="\xef\xbb\xbf" id="W5M0MpCehiHzreSzNTczkc9d"?>\n'
        '<x:xmpmeta xmlns:x="adobe:ns:meta/">\n'
        ' <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">\n'
        '  <rdf:Description rdf:about=""\n'
        '    xmlns:hdrgm="http://ns.adobe.com/hdr-gain-map/1.0/"\n'
        '    hdrgm:Version="1.0"\n'
        f'    hdrgm:GainMapMin="{metadata["GainMapMin"]}"\n'
        f'    hdrgm:GainMapMax="{metadata["GainMapMax"]}"\n'
        f'    hdrgm:Gamma="{metadata["Gamma"]}"\n'
        f'    hdrgm:OffsetSDR="{metadata["OffsetSDR"]}"\n'
        f'    hdrgm:OffsetHDR="{metadata["OffsetHDR"]}"\n'
        f'    hdrgm:HDRCapacityMin="{metadata["HDRCapacityMin"]}"\n'
        f'    hdrgm:HDRCapacityMax="{metadata["HDRCapacityMax"]}"\n'
        '    hdrgm:BaseRenditionIsHDR="False"/>\n'
        " </rdf:RDF>\n"
        "</x:xmpmeta>\n"
        '<?xpacket end="w"?>'
    )
    return xmp.encode("utf-8")


def build_mpf_payload(
    primary_size: int, gainmap_size: int, secondary_offset: int
) -> bytes:
    """Build MPF (Multi-Picture Format) APP2 payload (excl. FF E2 + length prefix).

    All IFD offsets are relative to the TIFF header (first byte after "MPF\\0").
    secondary_offset: byte offset of the gain map SOI from the file's first SOI.
    """
    num_entries = 3
    # IFD layout relative to TIFF header:
    #   8 bytes: TIFF header (byte order + magic + IFD offset)
    #   2 bytes: entry count
    #  36 bytes: 3 × 12-byte entries
    #   4 bytes: next IFD = 0
    # = 50 bytes to reach MPEntry data area
    mpentry_data_offset = 8 + 2 + 12 * num_entries + 4  # = 50

    buf = bytearray()
    # "MPF\0" identifier
    buf += b"MPF\x00"
    # TIFF header (big-endian)
    buf += b"\x4d\x4d\x00\x2a"
    # Offset to first IFD (relative to TIFF header)
    buf += struct.pack(">I", 8)
    # IFD entry count
    buf += struct.pack(">H", num_entries)
    # Entry 1: MPFVersion
    buf += struct.pack(">HHI", 0xB000, 7, 4) + b"0100"
    # Entry 2: NumberOfImages
    buf += struct.pack(">HHII", 0xB001, 4, 1, 2)
    # Entry 3: MPEntry (offset to MPEntry data, relative to TIFF header)
    buf += struct.pack(">HHII", 0xB002, 7, 32, mpentry_data_offset)
    # Next IFD = 0
    buf += struct.pack(">I", 0)
    # MPEntry 1: Primary image
    buf += struct.pack(">I", 0x00000000)  # Individual Image Attributes
    buf += struct.pack(">I", primary_size)  # Individual Image Size
    buf += struct.pack(">I", 0)  # Offset from first SOI = 0
    buf += b"\x00\x00\x00\x00"  # Dependent images
    # MPEntry 2: Gain map image
    buf += struct.pack(">I", 0x00000000)  # Individual Image Attributes
    buf += struct.pack(">I", gainmap_size)  # Individual Image Size
    buf += struct.pack(">I", secondary_offset)  # Offset from first SOI
    buf += b"\x00\x00\x00\x00"  # Dependent images

    return bytes(buf)


def build_iso_version_app2() -> bytes:
    """ISO 21496-1 version-only APP2 marker data (excl. FF E2 prefix)."""
    return ISO_NAMESPACE + struct.pack(">HH", 0, 0)


def _app_marker(marker_code: int, data: bytes) -> bytes:
    """Wrap data in a JPEG APP marker: FF xx + 2-byte length + data."""
    return bytes([0xFF, marker_code]) + struct.pack(">H", len(data) + 2) + data


def assemble_ultrahdr(
    primary_jpeg: bytes,
    gainmap_jpeg: bytes,
    metadata: dict,
) -> bytes:
    """Assemble Ultra HDR JPEG from primary JPEG + gain map JPEG + metadata."""
    # Extract EXIF from primary JPEG
    exif_segment = find_exif_in_jpeg(primary_jpeg)

    # Strip APP1 markers from primary to avoid duplicate EXIF/XMP
    primary_cleaned = strip_app1_markers(primary_jpeg)

    # Build gain map section components first to compute exact section size
    gainmap_xmp_data = XMP_NAMESPACE + build_gainmap_xmp(metadata)
    iso_version_data = build_iso_version_app2()

    # Gain map section = SOI + XMP APP1 + ISO APP2 + image data (without original SOI)
    gm_section_size = (
        2  # SOI
        + len(_app_marker(0xE1, gainmap_xmp_data))  # XMP APP1
        + len(_app_marker(0xE2, iso_version_data))  # ISO APP2
        + len(gainmap_jpeg)
        - 2  # image data (without original SOI)
    )

    # Build primary XMP with correct gain map section size
    primary_xmp_data = XMP_NAMESPACE + build_primary_xmp(gm_section_size)

    # Build placeholder MPF (offsets will be back-patched)
    mpf_payload = build_mpf_payload(0, gm_section_size, 0)
    mpf_marker = _app_marker(0xE2, mpf_payload)

    # ── Build the full output to measure exact offsets ──
    out = bytearray()

    # SOI
    out += b"\xff\xd8"

    # EXIF APP1 (preserved from original)
    exif_marker = b""
    if exif_segment:
        exif_marker = _app_marker(0xE1, exif_segment)
        out += exif_marker

    # XMP APP1 (GContainer + hdrgm:Version)
    xmp_marker = _app_marker(0xE1, primary_xmp_data)
    out += xmp_marker

    # ISO 21496-1 version APP2
    iso_marker = _app_marker(0xE2, iso_version_data)
    out += iso_marker

    # MPF APP2 (placeholder — will be patched)
    out += mpf_marker

    # Primary image data (SOI already stripped)
    out += primary_cleaned

    # secondary_offset = offset of second SOI from first SOI (= current length of out)
    secondary_offset = len(out)

    # ── Gain map section ──
    out += b"\xff\xd8"

    # XMP APP1 (gain map metadata)
    out += _app_marker(0xE1, gainmap_xmp_data)

    # ISO 21496-1 version APP2 (gain map)
    out += _app_marker(0xE2, iso_version_data)

    # Gain map image data (SOI stripped)
    out += gainmap_jpeg[2:]

    # ── Back-patch MPF with correct offsets ──
    # primary_size = bytes from first SOI to just before second SOI
    primary_size = secondary_offset
    # gainmap section size = second SOI + markers + gainmap image data
    gainmap_section_size = len(out) - secondary_offset
    mpf_payload = build_mpf_payload(
        primary_size, gainmap_section_size, secondary_offset
    )
    mpf_marker_patched = _app_marker(0xE2, mpf_payload)

    # Find and replace the placeholder MPF marker in out
    mpf_start = 2 + len(exif_marker) + len(xmp_marker) + len(iso_marker)
    mpf_end = mpf_start + len(mpf_marker)
    assert out[mpf_start:mpf_end] == mpf_marker, "MPF marker position mismatch"
    out[mpf_start:mpf_end] = mpf_marker_patched

    return bytes(out)


# ── Main ───────────────────────────────────────────────────────────────────────


def main():
    parser = argparse.ArgumentParser(
        description="RAW → Ultra HDR converter (feasibility demo)"
    )
    parser.add_argument("input", help="Input RAW file path")
    parser.add_argument(
        "-o", "--output", help="Output path (default: <input>_UltraHDR.jpg)"
    )
    parser.add_argument(
        "--half-size", action="store_true", help="Decode RAW at half size (faster)"
    )
    parser.add_argument(
        "--gainmap-quality", type=int, default=75, help="Gain map JPEG quality (0-100)"
    )
    args = parser.parse_args()

    output_path = args.output or args.input.rsplit(".", 1)[0] + "_UltraHDR.jpg"

    print(f"Input:  {args.input}")
    print(f"Output: {output_path}")
    print()

    # Step 1: Extract embedded JPEG
    print("[1/6] Extracting embedded JPEG preview...")
    embedded_jpeg = extract_embedded_jpeg(args.input)
    sdr_image = np.array(Image.open(io.BytesIO(embedded_jpeg)))
    print(f"      Size: {sdr_image.shape[1]}x{sdr_image.shape[0]}")

    # Step 2: Decode RAW to linear HDR
    print("[2/6] Decoding RAW to linear HDR (this may take a few seconds)...")
    hdr_linear = decode_raw_linear(args.input, half_size=args.half_size)
    print(
        f"      Size: {hdr_linear.shape[1]}x{hdr_linear.shape[0]}, dtype={hdr_linear.dtype}"
    )

    # Step 3: Compute gain map
    # full → 1:1 resolution, half → 1:4 resolution
    gainmap_scale = 4 if args.half_size else 1
    print("[3/6] Computing gain map...")
    gainmap, metadata = compute_gainmap(hdr_linear, sdr_image, scale=gainmap_scale)
    print(f"      Gain map: {gainmap.shape[1]}x{gainmap.shape[0]}")
    print(
        f"      GainMapMin={metadata['GainMapMin']:.4f}, GainMapMax={metadata['GainMapMax']:.4f}"
    )

    # Step 4: Encode gain map JPEG
    print("[4/6] Encoding gain map JPEG...")
    gainmap_jpeg = encode_gainmap_jpeg(gainmap, quality=args.gainmap_quality)
    print(f"      Size: {len(gainmap_jpeg)} bytes")

    # Step 5: Assemble Ultra HDR container
    print("[5/6] Assembling Ultra HDR JPEG...")
    output = assemble_ultrahdr(embedded_jpeg, gainmap_jpeg, metadata)
    print(f"      Total: {len(output)} bytes ({len(output) / 1024 / 1024:.1f} MB)")

    # Step 6: Write output
    print(f"[6/6] Writing {output_path}...")
    with open(output_path, "wb") as f:
        f.write(output)

    print()
    print("Done!")
    print()
    print("Verification:")
    print("  - Open on Android 14+ device (Google Photos should show HDR badge)")
    print("  - Open in Google Chrome 116+ on HDR display")
    print("  - Use exiftool: exiftool -GainMap* " + output_path)


if __name__ == "__main__":
    main()
