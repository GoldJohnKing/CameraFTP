#!/usr/bin/env python3
"""Diagnose v2: verify tone-curve-derived gain map produces no darkening."""

import io
import sys

import numpy as np
import rawpy
from PIL import Image

sys.path.insert(0, ".")
from main import (
    srgb_to_linear, _center_crop, _downsample_avg,
    derive_tone_curve, LUM_R, LUM_G, LUM_B,
    EPSILON_SDR, EPSILON_HDR,
)


def main():
    raw_path = sys.argv[1] if len(sys.argv) > 1 else "hdr_example/Nikon.NEF"

    # Extract embedded JPEG
    print("=" * 70)
    print("STEP 1: Extract embedded JPEG")
    print("=" * 70)
    with rawpy.imread(raw_path) as raw:
        thumb = raw.extract_thumb()
        embedded_jpeg = thumb.data
    sdr_image = np.array(Image.open(io.BytesIO(embedded_jpeg)))
    sdr_h, sdr_w = sdr_image.shape[:2]
    print(f"  SDR: {sdr_w}x{sdr_h}")

    # Decode RAW
    print()
    print("=" * 70)
    print("STEP 2: Decode RAW to linear HDR")
    print("=" * 70)
    with rawpy.imread(raw_path) as raw:
        hdr_linear = raw.postprocess(
            output_bps=16, gamma=(1, 1), no_auto_bright=True,
            use_camera_wb=True, output_color=rawpy.ColorSpace.sRGB,
        )
    hdr_h, hdr_w = hdr_linear.shape[:2]
    print(f"  HDR: {hdr_w}x{hdr_h}")

    # Align + compute luminance
    hdr_f = hdr_linear.astype(np.float64) / 65535.0
    if hdr_h != sdr_h or hdr_w != sdr_w:
        hdr_f = _center_crop(hdr_f, sdr_h, sdr_w)
        print(f"  Center-cropped HDR to {sdr_w}x{sdr_h}")

    Y_hdr = LUM_R * hdr_f[:,:,0] + LUM_G * hdr_f[:,:,1] + LUM_B * hdr_f[:,:,2]
    sdr_lin = srgb_to_linear(sdr_image.astype(np.float64) / 255.0)
    Y_sdr = LUM_R * sdr_lin[:,:,0] + LUM_G * sdr_lin[:,:,1] + LUM_B * sdr_lin[:,:,2]

    # Derive tone curve
    print()
    print("=" * 70)
    print("STEP 3: Derive tone curve")
    print("=" * 70)
    tone_lut = derive_tone_curve(Y_hdr.ravel(), Y_sdr.ravel())
    num_primary = len(tone_lut) - 256
    hdr_max_observed = float(np.percentile(Y_hdr, 99.5))
    bin_width = hdr_max_observed / num_primary
    print(f"  Primary bins: {num_primary}, Extrapolation bins: 256")
    print(f"  HDR 99.5th percentile: {hdr_max_observed:.4f}")
    print(f"  Bin width: {bin_width:.6f}")
    print(f"  Tone curve at key points:")
    for frac in [0.0, 0.1, 0.25, 0.5, 0.75, 0.9, 1.0]:
        idx = int(frac * (num_primary - 1))
        hdr_val = (idx + 0.5) * bin_width
        print(f"    Y_hdr={hdr_val:.4f} → Y_sdr={tone_lut[idx]:.4f}")
    # Extrapolation region
    for frac in [1.1, 1.25, 1.5, 2.0]:
        idx = num_primary + int((frac - 1.0) / (256 / num_primary))
        idx = min(idx, len(tone_lut) - 1)
        hdr_val = (idx + 0.5) * bin_width
        print(f"    Y_hdr={hdr_val:.4f} → Y_sdr={tone_lut[idx]:.4f} (extrapolated)")

    # Downsample and apply tone curve
    print()
    print("=" * 70)
    print("STEP 4: Gain map (same-domain comparison)")
    print("=" * 70)
    scale = 4
    gm_h, gm_w = sdr_h // scale, sdr_w // scale
    Y_hdr_down = _downsample_avg(Y_hdr, gm_h, gm_w)

    lut_indices = np.clip(Y_hdr_down / bin_width, 0, len(tone_lut) - 1.001)
    lut_pos = lut_indices.astype(np.intp)
    lut_frac = lut_indices - lut_pos
    Y_tonemapped = (
        tone_lut[lut_pos] * (1.0 - lut_frac)
        + tone_lut[np.minimum(lut_pos + 1, len(tone_lut) - 1)] * lut_frac
    )
    sdr_rendered = np.clip(Y_tonemapped, 0.0, 1.0)
    hdr_rendered = Y_tonemapped

    gain = (hdr_rendered + EPSILON_HDR) / (sdr_rendered + EPSILON_SDR)
    log_gain = np.log2(np.clip(gain, 1e-10, None))

    total = log_gain.size
    near_zero = np.abs(log_gain) < 0.01
    positive = log_gain > 0.01
    negative = log_gain < -0.01

    print(f"  Total pixels: {total:,}")
    print(f"  Near-zero gain (|log2| < 0.01): {near_zero.sum():,} ({100*near_zero.sum()/total:.1f}%)")
    print(f"  Positive gain (> 0.01):         {positive.sum():,} ({100*positive.sum()/total:.1f}%)")
    print(f"  Negative gain (< -0.01):        {negative.sum():,} ({100*negative.sum()/total:.1f}%)")
    print()
    print(f"  Percentiles:")
    for p in [0, 1, 5, 25, 50, 75, 95, 99, 100]:
        print(f"    P{p:02d}: {np.percentile(log_gain, p):.6f}")

    max_log = float(max(np.percentile(log_gain, 99), 0.01))
    recovery = np.clip(log_gain / max_log, 0.0, 1.0)
    gainmap = np.round(recovery * 255).astype(np.uint8)

    print()
    print(f"  GainMapMin = 0.0 (forced)")
    print(f"  GainMapMax = {max_log:.6f}")
    print(f"  Gain map histogram:")
    hist, _ = np.histogram(gainmap.ravel(), bins=range(257))
    nonzero_bins = [(i, hist[i]) for i in range(256) if hist[i] > 0]
    # Show top 10 bins by count
    for val, count in sorted(nonzero_bins, key=lambda x: -x[1])[:15]:
        print(f"    value={val:3d}: {count:>12,}  {'█' * int(60 * count / total)}")

    # Simulated HDR output
    print()
    print("=" * 70)
    print("STEP 5: Simulated HDR brightness")
    print("=" * 70)
    for weight in [0.0, 0.25, 0.5, 0.75, 1.0]:
        applied = log_gain * weight
        hdr_result = (sdr_rendered + EPSILON_SDR) * np.power(2.0, applied) - EPSILON_HDR
        hdr_result = np.clip(hdr_result, 0, None)
        ratio = hdr_result.mean() / Y_sdr.mean() if Y_sdr.mean() > 0 else 0
        print(f"  weight={weight:.2f}: HDR/SDR = {ratio:.4f}  "
              f"(SDR_mean={Y_sdr.mean():.4f}, HDR_mean={hdr_result.mean():.4f})")

    # Comparison with old approach
    print()
    print("=" * 70)
    print("COMPARISON: Old (direct) vs New (tone curve)")
    print("=" * 70)
    old_gain = (Y_hdr_down + EPSILON_HDR) / (_downsample_avg(Y_sdr, gm_h, gm_w) + EPSILON_SDR)
    old_log = np.log2(np.clip(old_gain, 1e-10, None))
    old_negative = (old_log < -0.01).sum()
    print(f"  Old approach: negative gain = {100*old_negative/total:.1f}%, "
          f"P50={np.median(old_log):.4f}, GainMapMin={np.percentile(old_log, 1):.4f}")
    print(f"  New approach: negative gain = {100*negative.sum()/total:.1f}%, "
          f"P50={np.median(log_gain):.4f}, GainMapMin=0.0")


if __name__ == "__main__":
    main()
