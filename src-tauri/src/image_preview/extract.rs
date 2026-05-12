// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::Path;

use memchr::memchr;

/// Extract the embedded JPEG preview from a RAW file by binary scanning
/// for the largest complete JPEG segment (SOI...EOI).
///
/// Camera RAW files always contain an embedded JPEG preview.
/// The largest JPEG segment is the full-size preview image.
/// This extracts the raw JPEG bytes directly — zero quality loss, no re-encoding.
///
/// If the extracted JPEG lacks EXIF metadata (common for Nikon NEF, Sony ARW, etc.),
/// the Orientation tag from the RAW file's TIFF/EXIF structure is injected as a
/// minimal APP1 segment so that downstream consumers (browsers, image viewers)
/// can apply the correct rotation.
pub fn extract_preview_jpeg(path: &Path) -> Result<Vec<u8>, String> {
    let path_str = path.to_string_lossy();
    tracing::debug!("Extracting RAW preview from: {}", path_str);

    let data =
        std::fs::read(path).map_err(|e| format!("Failed to read {}: {}", path_str, e))?;

    let mut jpeg = find_largest_jpeg(&data)
        .ok_or_else(|| format!("No embedded JPEG found in {}", path_str))?;

    tracing::debug!(
        "Extracted embedded JPEG: {} bytes from {}",
        jpeg.len(),
        path_str
    );

    // Camera-embedded preview JPEGs often omit EXIF metadata entirely.
    // Read the Orientation from the RAW file's TIFF structure and inject it.
    if !crate::image_utils::has_exif_app1(&jpeg) {
        if let Some(orientation) = read_raw_orientation(path) {
            if orientation != 1 {
                tracing::debug!(
                    "Injecting EXIF orientation {} into JPEG from {}",
                    orientation,
                    path_str
                );
                jpeg = crate::image_utils::inject_orientation_exif(jpeg, orientation);
            }
        }
    }

    Ok(jpeg)
}

/// Read the EXIF Orientation tag value from a RAW file using the shared parser.
fn read_raw_orientation(path: &Path) -> Option<u8> {
    crate::image_utils::parse_exif(path).ok()??.orientation
}

/// Scan binary data for the largest complete JPEG segment.
/// JPEG segments are delimited by SOI (0xFF 0xD8) and EOI (0xFF 0xD9) markers.
/// Uses memchr for SIMD-accelerated 0xFF byte scanning.
fn find_largest_jpeg(data: &[u8]) -> Option<Vec<u8>> {
    let mut best_start = 0;
    let mut best_size = 0;

    let len = data.len();
    if len < 4 {
        return None;
    }

    let mut i = 0;
    while i < len - 1 {
        // Skip to next 0xFF byte using SIMD-accelerated memchr
        let relative = match memchr(0xFF, &data[i..len - 1]) {
            Some(p) => p,
            None => break,
        };
        let ff_pos = i + relative;

        if data[ff_pos + 1] == 0xD8 {
            // Found SOI marker — scan for matching EOI
            let start = ff_pos;
            let mut j = start + 2;
            while j < len - 1 {
                let rel = match memchr(0xFF, &data[j..len - 1]) {
                    Some(p) => j + p,
                    None => break,
                };
                if data[rel + 1] == 0xD9 {
                    let size = rel + 2 - start;
                    if size > best_size {
                        best_start = start;
                        best_size = size;
                    }
                    break;
                }
                j = rel + 1;
            }
            i = ff_pos + 2;
        } else {
            i = ff_pos + 1;
        }
    }

    if best_size > 0 {
        Some(data[best_start..best_start + best_size].to_vec())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image_utils::{build_orientation_app1, has_exif_app1, inject_orientation_exif};

    #[test]
    fn build_orientation_app1_has_correct_structure() {
        let app1 = build_orientation_app1(8);
        // APP1 marker
        assert_eq!(app1[0], 0xFF);
        assert_eq!(app1[1], 0xE1);
        // Length = 34 (0x0022)
        assert_eq!(app1[2], 0x00);
        assert_eq!(app1[3], 0x22);
        // "Exif\0\0"
        assert_eq!(&app1[4..10], b"Exif\x00\x00");
        // Little-endian TIFF
        assert_eq!(&app1[10..12], b"II");
        // TIFF magic 42
        assert_eq!(u16::from_le_bytes([app1[12], app1[13]]), 42);
        // 1 IFD entry
        assert_eq!(u16::from_le_bytes([app1[18], app1[19]]), 1);
        // Tag = 0x0112 (Orientation)
        assert_eq!(u16::from_le_bytes([app1[20], app1[21]]), 0x0112);
        // Type = SHORT (3)
        assert_eq!(u16::from_le_bytes([app1[22], app1[23]]), 3);
        // Count = 1
        assert_eq!(u32::from_le_bytes([app1[24], app1[25], app1[26], app1[27]]), 1);
        // Value = 8
        assert_eq!(app1[28], 8);
    }

    #[test]
    fn inject_inserts_after_soi() {
        // SOI + DQT marker (minimal fake JPEG)
        let jpeg = vec![0xFF, 0xD8, 0xFF, 0xDB, 0x00, 0x01, 0x00];
        let result = inject_orientation_exif(jpeg.clone(), 6);
        // SOI
        assert_eq!(&result[..2], &[0xFF, 0xD8]);
        // APP1 marker follows immediately
        assert_eq!(&result[2..4], &[0xFF, 0xE1]);
        // Original DQT still present after injected segment
        // APP1 length field at result[4..6] gives length of (length field + payload)
        let app1_len = u16::from_be_bytes([result[4], result[5]]) as usize;
        // APP1 total = 2 (marker) + app1_len (length field + payload)
        let app1_total = 2 + app1_len;
        assert_eq!(&result[2 + app1_total..2 + app1_total + 2], &[0xFF, 0xDB]);
    }

    #[test]
    fn has_exif_app1_detects_presence() {
        // JPEG with APP1
        let mut with_exif = vec![0xFF, 0xD8, 0xFF, 0xE1, 0x00, 0x08];
        with_exif.extend(b"Exif\x00\x00");
        with_exif.extend(vec![0xFF, 0xD9]);
        assert!(has_exif_app1(&with_exif));

        // JPEG without APP1 (DQT immediately after SOI)
        let without_exif = vec![0xFF, 0xD8, 0xFF, 0xDB, 0x00, 0x01, 0x00];
        assert!(!has_exif_app1(&without_exif));
    }

    #[test]
    fn find_largest_jpeg_returns_largest() {
        // Small JPEG: SOI + 2 payload bytes + EOI = 6 bytes
        let small: Vec<u8> = vec![0xFF, 0xD8, 0xAA, 0xBB, 0xFF, 0xD9];
        // Large JPEG: SOI + 10 payload bytes + EOI = 14 bytes
        let large: Vec<u8> = vec![
            0xFF, 0xD8, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0xFF, 0xD9,
        ];

        // Place small JPEG first, then some filler, then large JPEG
        let mut data = Vec::new();
        data.extend_from_slice(&small);
        data.extend_from_slice(&[0xDE, 0xAD]); // non-JPEG filler
        data.extend_from_slice(&large);

        let result = find_largest_jpeg(&data).expect("should find a JPEG");
        assert_eq!(result.len(), 14);
        assert_eq!(result, large);
    }

    #[test]
    fn find_largest_jpeg_returns_none_for_no_jpeg() {
        let data: Vec<u8> = vec![0x00, 0x01, 0x02, 0x03, 0x04, 0x05];
        assert!(find_largest_jpeg(&data).is_none());
    }

    #[test]
    fn find_largest_jpeg_returns_none_for_empty() {
        assert!(find_largest_jpeg(&[]).is_none());
    }

    #[test]
    fn find_largest_jpeg_handles_soi_without_eoi() {
        let data: Vec<u8> = vec![0xFF, 0xD8, 0x00, 0x01, 0x02, 0x03];
        assert!(find_largest_jpeg(&data).is_none());
    }
}
