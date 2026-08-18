// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Shared utilities for image format detection and EXIF metadata parsing.
//!
//! This module is the single source of truth for:
//! - RAW file extension detection (`is_raw_file`, `is_supported_image`)
//! - EXIF metadata extraction (`parse_exif`)
//! - EXIF orientation injection into JPEG files

use std::path::Path;
use chrono::NaiveDateTime;
use nom_exif::URational;

/// All supported RAW image file extensions (lowercase).
pub const RAW_EXTENSIONS: &[&str] = &[
    "nef", "nrw", "cr2", "cr3", "arw", "sr2",
    "raf", "orf", "rw2", "pef", "dng", "x3f", "raw", "srw",
];
// NOTE: Keep in sync with src/utils/raw.ts (TypeScript side).

/// Check if a file path has a RAW image extension.
pub fn is_raw_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| RAW_EXTENSIONS.iter().any(|ext| ext.eq_ignore_ascii_case(e)))
        .unwrap_or(false)
}

/// Check if a file path is a supported image format (JPEG/HEIF + all RAW).
pub fn is_supported_image(path: &Path) -> bool {
    if is_raw_file(path) {
        return true;
    }
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            let ext = e.to_lowercase();
            matches!(ext.as_str(), "jpg" | "jpeg" | "heif" | "hif" | "heic")
        })
        .unwrap_or(false)
}

/// Raw EXIF values parsed from an image file.
///
/// Callers extract the fields they need and apply their own formatting.
/// Rational values use `nom_exif::URational` (`Rational<u32>`) — access
/// numerator/denominator via `.0` and `.1`.
#[derive(Debug)]
pub struct ParsedExif {
    pub iso: Option<u32>,
    pub aperture: Option<URational>,
    pub shutter_speed: Option<URational>,
    pub focal_length_35mm: Option<u16>,
    pub focal_length_raw: Option<URational>,
    pub datetime_original: Option<NaiveDateTime>,
    pub orientation: Option<u8>,
}

/// Parse EXIF metadata from any supported image file.
///
/// Returns `Ok(None)` if the file has no EXIF data.
/// Returns `Err(String)` only if the file cannot be opened.
/// Parse failures are logged and return `Ok(None)` to avoid disrupting callers.
pub fn parse_exif(path: &Path) -> Result<Option<ParsedExif>, String> {
    use nom_exif::*;

    let path_str = path.to_string_lossy();

    let mut parser = MediaParser::new();
    let ms = MediaSource::file_path(path)
        .map_err(|e| format!("Failed to open file for EXIF {}: {}", path_str, e))?;

    if !ms.has_exif() {
        return Ok(None);
    }

    let iter: ExifIter = match parser.parse(ms) {
        Ok(iter) => iter,
        Err(e) => {
            tracing::warn!("Failed to parse EXIF for {}: {:?}", path_str, e);
            return Ok(None);
        }
    };

    let exif: Exif = iter.into();

    Ok(Some(ParsedExif {
        iso: exif.get(ExifTag::ISOSpeedRatings)
            .and_then(|v| v.as_u16())
            .map(|v| v as u32),
        aperture: exif.get(ExifTag::FNumber)
            .and_then(|v| v.as_urational()),
        shutter_speed: exif.get(ExifTag::ExposureTime)
            .and_then(|v| v.as_urational()),
        focal_length_35mm: exif.get(ExifTag::FocalLengthIn35mmFilm)
            .and_then(|v| v.as_u16()),
        focal_length_raw: exif.get(ExifTag::FocalLength)
            .and_then(|v| v.as_urational()),
        datetime_original: exif.get(ExifTag::DateTimeOriginal)
            .and_then(|v| v.as_time_components())
            .map(|(ndt, _offset)| ndt),
        orientation: exif.get(ExifTag::Orientation)
            .and_then(|v| v.as_u16())
            .map(|v| v as u8),
    }))
}

/// Check if a JPEG already has an APP1/EXIF marker.
pub fn has_exif_app1(jpeg: &[u8]) -> bool {
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
            let seg_len = u16::from_be_bytes([jpeg[i + 2], jpeg[i + 3]]) as usize;
            if i + 4 + 6 <= jpeg.len() && &jpeg[i + 4..i + 10] == b"Exif\x00\x00" {
                return true;
            }
            i += 2 + seg_len;
            continue;
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
pub fn build_orientation_app1(orientation: u8) -> Vec<u8> {
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

/// Inject a minimal EXIF APP1 segment with an Orientation tag into a JPEG.
/// Inserted right after the SOI marker (FF D8).
pub fn inject_orientation_exif(jpeg: Vec<u8>, orientation: u8) -> Vec<u8> {
    let app1 = build_orientation_app1(orientation);
    let mut result = Vec::with_capacity(jpeg.len() + app1.len());
    result.extend_from_slice(&jpeg[..2]); // SOI
    result.extend(app1);
    result.extend_from_slice(&jpeg[2..]); // Rest of JPEG
    result
}

/// Builds a full APP1/EXIF segment whose IFD0 carries `Orientation` and whose
/// Exif sub-IFD carries `DateTimeOriginal` (`datetime`, "YYYY:MM:DD HH:MM:SS").
///
/// Test-only counterpart of [`build_orientation_app1`] for exercising
/// [`parse_exif`] end-to-end against a real EXIF payload.
///
/// Layout (little-endian TIFF, offsets relative to TIFF header start):
///   0   "II" 2A00 08000000   TIFF header, IFD0 at 8
///   8   IFD0 (2 entries: Orientation, ExifIFDPointer)
///   38  Exif IFD (1 entry: DateTimeOriginal)
///   56  20-byte ASCII datetime field (19 chars + NUL)
#[cfg(test)]
pub(crate) fn build_exif_datetime_orientation_app1(datetime: &str, orientation: u16) -> Vec<u8> {
    let dt = datetime.as_bytes();
    assert_eq!(dt.len(), 19, "datetime must be formatted YYYY:MM:DD HH:MM:SS");

    const IFD0_OFFSET: u32 = 8;
    const IFD0_LEN: u32 = 2 + 2 * 12 + 4; // entry count + 2 entries + next-IFD
    const EXIF_IFD_OFFSET: u32 = IFD0_OFFSET + IFD0_LEN; // 38
    const EXIF_IFD_LEN: u32 = 2 + 1 * 12 + 4; // entry count + 1 entry + next-IFD
    const DATETIME_OFFSET: u32 = EXIF_IFD_OFFSET + EXIF_IFD_LEN; // 56

    let mut tiff: Vec<u8> = Vec::with_capacity(DATETIME_OFFSET as usize + 20);
    tiff.extend_from_slice(b"II"); // little-endian byte order
    tiff.extend_from_slice(&42u16.to_le_bytes()); // TIFF magic
    tiff.extend_from_slice(&IFD0_OFFSET.to_le_bytes());

    // IFD0 — entries ascending by tag (0x0112 < 0x8769)
    tiff.extend_from_slice(&2u16.to_le_bytes());
    // Orientation (0x0112), SHORT, count 1, inline value
    tiff.extend_from_slice(&0x0112u16.to_le_bytes());
    tiff.extend_from_slice(&3u16.to_le_bytes());
    tiff.extend_from_slice(&1u32.to_le_bytes());
    tiff.extend_from_slice(&orientation.to_le_bytes());
    tiff.extend_from_slice(&[0u8, 0u8]); // pad inline value to 4 bytes
    // ExifIFDPointer (0x8769), LONG, count 1
    tiff.extend_from_slice(&0x8769u16.to_le_bytes());
    tiff.extend_from_slice(&4u16.to_le_bytes());
    tiff.extend_from_slice(&1u32.to_le_bytes());
    tiff.extend_from_slice(&EXIF_IFD_OFFSET.to_le_bytes());
    tiff.extend_from_slice(&0u32.to_le_bytes()); // no next IFD

    // Exif sub-IFD
    tiff.extend_from_slice(&1u16.to_le_bytes());
    // DateTimeOriginal (0x9003), ASCII, count 20 (19 chars + NUL)
    tiff.extend_from_slice(&0x9003u16.to_le_bytes());
    tiff.extend_from_slice(&2u16.to_le_bytes());
    tiff.extend_from_slice(&20u32.to_le_bytes());
    tiff.extend_from_slice(&DATETIME_OFFSET.to_le_bytes());
    tiff.extend_from_slice(&0u32.to_le_bytes()); // no next IFD
    tiff.extend_from_slice(dt);
    tiff.push(0);

    let payload_len = 6 + tiff.len(); // "Exif\0\0" + TIFF
    let mut segment = Vec::with_capacity(4 + payload_len);
    segment.extend_from_slice(&[0xFF, 0xE1]); // APP1 marker
    segment.extend_from_slice(&((payload_len + 2) as u16).to_be_bytes()); // segment length
    segment.extend_from_slice(b"Exif\0\0");
    segment.extend_from_slice(&tiff);
    segment
}

/// Encodes a tiny real JPEG (via the `image` crate) with the EXIF APP1 segment
/// from [`build_exif_datetime_orientation_app1`] injected after SOI.
#[cfg(test)]
pub(crate) fn build_exif_jpeg(datetime: &str, orientation: u16) -> Vec<u8> {
    let img = image::RgbImage::from_pixel(2, 2, image::Rgb([128u8, 128, 128]));
    let mut jpeg = Vec::new();
    image::DynamicImage::ImageRgb8(img)
        .write_to(&mut std::io::Cursor::new(&mut jpeg), image::ImageFormat::Jpeg)
        .expect("failed to encode test JPEG");

    let app1 = build_exif_datetime_orientation_app1(datetime, orientation);
    let mut out = Vec::with_capacity(jpeg.len() + app1.len());
    out.extend_from_slice(&jpeg[..2]); // SOI
    out.extend(app1);
    out.extend_from_slice(&jpeg[2..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_raw_file() {
        assert!(is_raw_file(Path::new("photo.nef")));
        assert!(is_raw_file(Path::new("photo.CR3")));
        assert!(is_raw_file(Path::new("photo.ARW")));
        assert!(is_raw_file(Path::new("photo.dng")));
        assert!(!is_raw_file(Path::new("photo.jpg")));
        assert!(!is_raw_file(Path::new("photo.jpeg")));
        assert!(!is_raw_file(Path::new("photo.png")));
        assert!(!is_raw_file(Path::new("photo")));
        assert!(!is_raw_file(Path::new("photo.txt")));
    }

    #[test]
    fn test_is_supported_image() {
        assert!(is_supported_image(Path::new("photo.jpg")));
        assert!(is_supported_image(Path::new("photo.JPEG")));
        assert!(is_supported_image(Path::new("photo.heif")));
        assert!(is_supported_image(Path::new("photo.nef")));
        assert!(is_supported_image(Path::new("photo.cr2")));
        assert!(!is_supported_image(Path::new("photo.png")));
        assert!(!is_supported_image(Path::new("photo.mp4")));
        assert!(!is_supported_image(Path::new("photo.txt")));
    }

    #[test]
    fn test_has_exif_app1() {
        // JPEG with APP1/EXIF marker including "Exif\0\0" header
        let jpeg_with_exif = vec![
            0xFF, 0xD8, 0xFF, 0xE1, 0x00, 0x0A,
            b'E', b'x', b'i', b'f', 0x00, 0x00, 0x00, 0x00,
        ];
        assert!(has_exif_app1(&jpeg_with_exif));

        // JPEG with APP1 marker but XMP (not EXIF) — no "Exif\0\0" header
        let jpeg_with_xmp = vec![
            0xFF, 0xD8, 0xFF, 0xE1, 0x00, 0x04,
            b'h', b't', b'm', b'l',
        ];
        assert!(!has_exif_app1(&jpeg_with_xmp));

        // JPEG without APP1 (SOI + APP0/DQT marker instead)
        let jpeg_without_app1 = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0x00, 0x00];
        assert!(!has_exif_app1(&jpeg_without_app1));

        // Too short to contain a valid JPEG
        let too_short = vec![0xFF, 0xD8];
        assert!(!has_exif_app1(&too_short));

        // Not a JPEG at all
        let not_jpeg = vec![0x89, 0x50, 0x4E, 0x47];
        assert!(!has_exif_app1(&not_jpeg));
    }

    #[test]
    fn test_build_orientation_app1_length() {
        for orientation in [1u8, 2, 3, 4, 5, 6, 7, 8] {
            let app1 = build_orientation_app1(orientation);
            assert_eq!(app1.len(), 36, "APP1 segment should be 36 bytes for orientation {}", orientation);
        }
    }

    #[test]
    fn test_inject_orientation_exif_preserves_original() {
        // Minimal valid JPEG: SOI + some data
        let original = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x02, 0xAA, 0xBB];

        let result = inject_orientation_exif(original.clone(), 6);

        // SOI is preserved at the start
        assert_eq!(&result[0..2], &[0xFF, 0xD8]);

        // APP1 segment follows SOI (starts with FFE1)
        assert_eq!(&result[2..4], &[0xFF, 0xE1]);

        // Original data after SOI is preserved after the injected APP1
        let app1_len = 36;
        assert_eq!(&result[2 + app1_len..], &original[2..]);

        // Total length = original + 36 (the injected APP1 segment)
        assert_eq!(result.len(), original.len() + app1_len);
    }

    // ---- parse_exif tests against a real (tiny) EXIF-bearing JPEG ----

    #[test]
    fn parse_exif_extracts_datetime_and_orientation_from_real_jpeg() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("exif.jpg");
        std::fs::write(&path, build_exif_jpeg("2024:01:15 10:30:00", 6)).unwrap();

        let parsed = parse_exif(&path)
            .expect("existing file must open")
            .expect("EXIF-bearing JPEG must yield Some");
        let expected = chrono::NaiveDate::from_ymd_opt(2024, 1, 15)
            .unwrap()
            .and_hms_opt(10, 30, 0)
            .unwrap();
        assert_eq!(parsed.datetime_original, Some(expected));
        assert_eq!(parsed.orientation, Some(6));

        // Tags absent from the fixture must be None, not garbage
        assert_eq!(parsed.iso, None);
        assert_eq!(parsed.aperture, None);
        assert_eq!(parsed.shutter_speed, None);
        assert_eq!(parsed.focal_length_35mm, None);
        assert_eq!(parsed.focal_length_raw, None);
    }

    #[test]
    fn parse_exif_jpeg_without_exif_returns_ok_none() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("plain.jpg");

        let img = image::RgbImage::from_pixel(2, 2, image::Rgb([128u8, 128, 128]));
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut std::fs::File::create(&path).unwrap(), image::ImageFormat::Jpeg)
            .unwrap();

        assert!(
            matches!(parse_exif(&path), Ok(None)),
            "JPEG without EXIF must return Ok(None), got {:?}",
            parse_exif(&path)
        );
    }

    #[test]
    fn parse_exif_missing_file_returns_err() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("missing.jpg");
        assert!(
            parse_exif(&path).is_err(),
            "missing file must return Err (only open failures are errors)"
        );
    }
}
