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

/// Check if a file path has a RAW image extension.
pub fn is_raw_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| RAW_EXTENSIONS.contains(&e.to_lowercase().as_str()))
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
}
