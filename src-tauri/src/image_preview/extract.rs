// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::Path;

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
    if !has_exif_app1(&jpeg) {
        if let Some(orientation) = read_raw_orientation(path) {
            if orientation != 1 {
                tracing::debug!(
                    "Injecting EXIF orientation {} into JPEG from {}",
                    orientation,
                    path_str
                );
                jpeg = inject_orientation_exif(jpeg, orientation);
            }
        }
    }

    Ok(jpeg)
}

/// Read the EXIF Orientation tag value from a RAW file using nom-exif.
fn read_raw_orientation(path: &Path) -> Option<u8> {
    use nom_exif::*;

    let mut parser = MediaParser::new();
    let ms = MediaSource::file_path(path).ok()?;
    if !ms.has_exif() {
        return None;
    }
    let iter: ExifIter = parser.parse(ms).ok()?;
    let exif: Exif = iter.into();
    exif.get(ExifTag::Orientation)
        .and_then(|v| v.as_u16())
        .map(|v| v as u8)
}

/// Check if a JPEG already has an APP1/EXIF marker.
fn has_exif_app1(jpeg: &[u8]) -> bool {
    if jpeg.len() < 4 || jpeg[0] != 0xFF || jpeg[1] != 0xD8 {
        return false;
    }
    let mut i = 2;
    while i + 3 < jpeg.len() {
        if jpeg[i] != 0xFF {
            return false;
        }
        let marker = jpeg[i + 1];
        if marker == 0xE1 {
            return true;
        }
        if marker == 0xDA {
            return false;
        }
        if marker == 0x00 || (0xD0..=0xD9).contains(&marker) {
            i += 2;
            continue;
        }
        let seg_len = u16::from_be_bytes([jpeg[i + 2], jpeg[i + 3]]) as usize;
        i += 2 + seg_len;
    }
    false
}

/// Inject a minimal EXIF APP1 segment with an Orientation tag into a JPEG.
/// Inserted right after the SOI marker (FF D8).
fn inject_orientation_exif(jpeg: Vec<u8>, orientation: u8) -> Vec<u8> {
    let app1 = build_orientation_app1(orientation);
    let mut result = Vec::with_capacity(jpeg.len() + app1.len());
    result.extend_from_slice(&jpeg[..2]); // SOI
    result.extend(app1);
    result.extend_from_slice(&jpeg[2..]); // Rest of JPEG
    result
}

/// Build a minimal APP1/EXIF segment containing only the Orientation tag.
///
/// Structure:
///   FFE1              - APP1 marker
///   0022              - Length: 34 (2 + 32 payload)
///   "Exif\0\0"        - EXIF header (6 bytes)
///   II 2A00 08000000  - TIFF header: little-endian, magic 42, IFD0 at offset 8
///   0100              - 1 IFD entry
///   1201 0300 01000000 XX000000 - Orientation tag: SHORT, count=1, value=XX
///   00000000          - Next IFD: none
fn build_orientation_app1(orientation: u8) -> Vec<u8> {
    vec![
        0xFF, 0xE1, // APP1 marker
        0x00, 0x22, // Length: 34
        b'E', b'x', b'i', b'f', 0x00, 0x00, // "Exif\0\0"
        b'I', b'I', // Little-endian
        0x2A, 0x00, // TIFF magic: 42
        0x08, 0x00, 0x00, 0x00, // IFD0 offset: 8
        0x01, 0x00, // 1 IFD entry
        0x12, 0x01, // Tag: Orientation (0x0112)
        0x03, 0x00, // Type: SHORT
        0x01, 0x00, 0x00, 0x00, // Count: 1
        orientation, 0x00, 0x00, 0x00, // Value
        0x00, 0x00, 0x00, 0x00, // Next IFD: 0
    ]
}

/// Scan binary data for the largest complete JPEG segment.
/// JPEG segments are delimited by SOI (0xFF 0xD8) and EOI (0xFF 0xD9) markers.
fn find_largest_jpeg(data: &[u8]) -> Option<Vec<u8>> {
    let mut best_start = 0;
    let mut best_size = 0;

    let len = data.len();
    if len < 4 {
        return None;
    }

    let mut i = 0;
    while i < len - 1 {
        // Look for JPEG SOI marker
        if data[i] == 0xFF && data[i + 1] == 0xD8 {
            let start = i;
            // Find the corresponding EOI marker
            let mut j = i + 2;
            while j < len - 1 {
                if data[j] == 0xFF && data[j + 1] == 0xD9 {
                    let size = j + 2 - start;
                    if size > best_size {
                        best_start = start;
                        best_size = size;
                    }
                    break;
                }
                j += 1;
            }
            // Continue scanning from after this SOI
            i += 2;
        } else {
            i += 1;
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
        let app1_len = u16::from_be_bytes([result[4], result[5]]) as usize;
        assert_eq!(&result[2 + app1_len..2 + app1_len + 2], &[0xFF, 0xDB]);
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
}
