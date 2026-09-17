// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use memchr::memchr;

/// Fujifilm RAF magic prefix. A real RAF always begins with these 16 bytes.
const FUJI_MAGIC: &[u8] = b"FUJIFILMCCD-RAW ";
/// RAF header is fixed at 148 bytes across format versions 0100/0201/0202/0203.
const RAF_HEADER_LEN: usize = 148;
/// Byte offset of the embedded-JPEG offset field (big-endian u32) in the RAF header.
const RAF_JPEG_OFFSET_POS: usize = 0x54;
/// Byte offset of the embedded-JPEG length field (big-endian u32) in the RAF header.
const RAF_JPEG_LENGTH_POS: usize = 0x58;

/// Extract the embedded JPEG preview from a RAW file.
///
/// Fujifilm RAF files use a header-field fast-path: the RAF header stores the
/// JPEG offset/length at fixed positions (0x54/0x58), so we seek and read only
/// the embedded JPEG region — never the multi-MB CFA sensor payload. This is the
/// method used by exiftool/LibRaw/photorec and is robust where naive SOI/EOI
/// scanning can pick a wrong segment (RAF's huge CFA block defeats "largest
/// JPEG" heuristics, especially on compressed X-T3/T4/T5 variants).
///
/// All other RAW formats (NEF, CR2, ARW, ...) fall back to binary scanning for
/// the largest complete JPEG segment (SOI...EOI), which is reliable for their
/// TIFF-based containers.
///
/// The extracted bytes are the raw JPEG — zero quality loss, no re-encoding. If
/// the result lacks EXIF metadata, the Orientation tag is injected from the
/// RAW's TIFF structure so downstream viewers apply the correct rotation.
pub fn extract_preview_jpeg(path: &Path) -> Result<Vec<u8>, String> {
    let path_str = path.to_string_lossy();
    tracing::debug!("Extracting RAW preview from: {}", path_str);

    let metadata =
        std::fs::metadata(path).map_err(|e| format!("Failed to stat {}: {}", path_str, e))?;
    const MAX_RAW_SIZE: u64 = 100 * 1024 * 1024; // 100 MB
    if metadata.len() > MAX_RAW_SIZE {
        tracing::warn!(
            "RAW file too large for thumbnail extraction ({} MB): {}",
            metadata.len() / (1024 * 1024),
            path_str
        );
        return Err(format!(
            "RAW file too large for preview extraction ({} MB)",
            metadata.len() / (1024 * 1024)
        ));
    }

    // RAF fast-path first; everything else scans the whole file.
    let mut jpeg = match extract_raf_jpeg_fast(path, metadata.len()) {
        Ok(j) => j,
        Err(RafPathError::NotRaf) => {
            let data =
                std::fs::read(path).map_err(|e| format!("Failed to read {}: {}", path_str, e))?;
            find_largest_jpeg(&data)
                .ok_or_else(|| format!("No embedded JPEG found in {}", path_str))?
        }
        Err(RafPathError::HeaderInvalid(msg)) => {
            // RAF magic present but header fields unusable — fall back to scan,
            // which is still correct for well-formed RAFs.
            tracing::warn!(
                "RAF header parse failed ({}); falling back to full scan for {}",
                msg,
                path_str
            );
            let data =
                std::fs::read(path).map_err(|e| format!("Failed to read {}: {}", path_str, e))?;
            find_largest_jpeg(&data)
                .ok_or_else(|| format!("No embedded JPEG found in {}", path_str))?
        }
    };

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

#[derive(Debug)]
enum RafPathError {
    /// File does not carry the Fujifilm RAF magic — caller should use the scan path.
    NotRaf,
    /// File looks like a RAF but its header fields are unusable; scan is a safe fallback.
    HeaderInvalid(String),
}

/// RAF fast-path: read the 148-byte header, parse the JPEG offset/length fields
/// (big-endian u32 at 0x54/0x58), then seek and read only the embedded JPEG
/// region. Reads O(header + jpeg) bytes instead of O(file size), which matters
/// for RAFs whose CFA payload is tens of MB.
fn extract_raf_jpeg_fast(path: &Path, file_len: u64) -> Result<Vec<u8>, RafPathError> {
    let mut file = std::fs::File::open(path)
        .map_err(|e| RafPathError::HeaderInvalid(format!("open: {}", e)))?;

    // A real RAF always has a full 148-byte header. A short read here means the
    // file is not RAF (or is truncated); treat both as NotRaf so the caller scans.
    let mut header = [0u8; RAF_HEADER_LEN];
    if file.read_exact(&mut header).is_err() || &header[..16] != FUJI_MAGIC {
        return Err(RafPathError::NotRaf);
    }

    let off = u32::from_be_bytes([
        header[RAF_JPEG_OFFSET_POS],
        header[RAF_JPEG_OFFSET_POS + 1],
        header[RAF_JPEG_OFFSET_POS + 2],
        header[RAF_JPEG_OFFSET_POS + 3],
    ]) as u64;
    let len = u32::from_be_bytes([
        header[RAF_JPEG_LENGTH_POS],
        header[RAF_JPEG_LENGTH_POS + 1],
        header[RAF_JPEG_LENGTH_POS + 2],
        header[RAF_JPEG_LENGTH_POS + 3],
    ]) as u64;

    if len < 8 {
        return Err(RafPathError::HeaderInvalid(format!(
            "jpeg length too small: {}",
            len
        )));
    }
    let end = off
        .checked_add(len)
        .ok_or_else(|| RafPathError::HeaderInvalid("offset + length overflow".into()))?;
    if end > file_len {
        return Err(RafPathError::HeaderInvalid(format!(
            "jpeg region {}-{} extends beyond file len {}",
            off, end, file_len
        )));
    }

    file.seek(SeekFrom::Start(off))
        .map_err(|e| RafPathError::HeaderInvalid(format!("seek: {}", e)))?;
    let mut buf = vec![0u8; len as usize];
    file.read_exact(&mut buf)
        .map_err(|e| RafPathError::HeaderInvalid(format!("read jpeg region: {}", e)))?;

    // Validate a real JPEG begins here; otherwise the header lied — let caller scan.
    if buf.len() < 3 || buf[0] != 0xFF || buf[1] != 0xD8 || buf[2] != 0xFF {
        return Err(RafPathError::HeaderInvalid(
            "jpeg region does not start with SOI".into(),
        ));
    }

    tracing::debug!(
        "RAF fast-path: extracted {} bytes of embedded JPEG (offset {}, file {})",
        buf.len(),
        off,
        file_len
    );
    Ok(buf)
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

    if best_size >= 8 {
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
        assert_eq!(
            u32::from_le_bytes([app1[24], app1[25], app1[26], app1[27]]),
            1
        );
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

    // ── RAF header-field fast-path ──────────────────────────────────────────

    fn write_temp_raf(name: &str, data: &[u8]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("cameraftp_test_raf_extract");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, data).unwrap();
        path
    }

    /// Build a synthetic RAF: 148-byte header with JPEG offset/length filled in,
    /// a CFA filler region, then the embedded JPEG. `corrupt_soi` clobbers the
    /// first byte of the embedded region to defeat the JPEG-start validation.
    fn build_fake_raf(jpeg_offset: u32, jpeg: &[u8], corrupt_soi: bool) -> Vec<u8> {
        let mut buf = vec![0u8; RAF_HEADER_LEN];
        buf[..FUJI_MAGIC.len()].copy_from_slice(FUJI_MAGIC);
        buf[RAF_JPEG_OFFSET_POS..RAF_JPEG_OFFSET_POS + 4]
            .copy_from_slice(&jpeg_offset.to_be_bytes());
        buf[RAF_JPEG_LENGTH_POS..RAF_JPEG_LENGTH_POS + 4]
            .copy_from_slice(&(jpeg.len() as u32).to_be_bytes());
        // CFA filler between the header and the embedded JPEG.
        if (jpeg_offset as usize) > buf.len() {
            buf.resize(jpeg_offset as usize, 0xAA);
        }
        if corrupt_soi {
            let mut corrupt = jpeg.to_vec();
            corrupt[0] = 0x00;
            buf.extend_from_slice(&corrupt);
        } else {
            buf.extend_from_slice(jpeg);
        }
        buf
    }

    #[test]
    fn extract_raf_fast_parses_header_and_reads_only_jpeg_region() {
        let jpeg: Vec<u8> = {
            let mut v = vec![0xFF, 0xD8, 0xFF, 0xE0];
            v.extend(std::iter::repeat(0x42u8).take(60));
            v.extend_from_slice(&[0xFF, 0xD9]);
            v
        };
        let data = build_fake_raf(512, &jpeg, false);
        let path = write_temp_raf("raf_ok.raf", &data);

        let got = extract_raf_jpeg_fast(&path, data.len() as u64).expect("should extract");
        assert_eq!(got, jpeg);
        assert_eq!(&got[..3], &[0xFF, 0xD8, 0xFF]);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn extract_raf_fast_rejects_non_raf_magic() {
        let path = write_temp_raf("not_raf.raf", &vec![0u8; 256]);
        match extract_raf_jpeg_fast(&path, 256) {
            Err(RafPathError::NotRaf) => {}
            other => panic!("expected NotRaf, got {:?}", other.map(|v| v.len())),
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn extract_raf_fast_rejects_region_without_valid_soi() {
        let jpeg: Vec<u8> = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x02, 0x00, 0x00, 0xFF, 0xD9];
        let data = build_fake_raf(512, &jpeg, true);
        let path = write_temp_raf("raf_badsoi.raf", &data);

        match extract_raf_jpeg_fast(&path, data.len() as u64) {
            Err(RafPathError::HeaderInvalid(_)) => {}
            other => panic!("expected HeaderInvalid, got {:?}", other.map(|v| v.len())),
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn extract_raf_fast_rejects_region_beyond_eof() {
        // Header claims a JPEG length that runs past the real file end.
        let mut data = vec![0u8; RAF_HEADER_LEN];
        data[..FUJI_MAGIC.len()].copy_from_slice(FUJI_MAGIC);
        data[RAF_JPEG_OFFSET_POS..RAF_JPEG_OFFSET_POS + 4].copy_from_slice(&512u32.to_be_bytes());
        data[RAF_JPEG_LENGTH_POS..RAF_JPEG_LENGTH_POS + 4]
            .copy_from_slice(&1_000_000u32.to_be_bytes()); // far beyond EOF

        let path = write_temp_raf("raf_eof.raf", &data);
        match extract_raf_jpeg_fast(&path, data.len() as u64) {
            Err(RafPathError::HeaderInvalid(_)) => {}
            other => panic!("expected HeaderInvalid, got {:?}", other.map(|v| v.len())),
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn extract_preview_jpeg_takes_raf_fast_path() {
        // A RAF whose embedded JPEG is small and sits after filler. The fast path
        // returns exactly the header-pointed JPEG; whole-file scanning would also
        // find it, but this confirms the fast path is taken and the bytes match.
        let jpeg: Vec<u8> = {
            let mut v = vec![0xFF, 0xD8, 0xFF, 0xE0];
            v.extend(std::iter::repeat(0x7Fu8).take(40));
            v.extend_from_slice(&[0xFF, 0xD9]);
            v
        };
        let data = build_fake_raf(512, &jpeg, false);
        let path = write_temp_raf("raf_e2e.raf", &data);

        let got = extract_preview_jpeg(&path).expect("should extract via fast path");
        assert_eq!(got, jpeg);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn extract_preview_jpeg_falls_back_to_scan_for_non_raf() {
        // Non-RAF binary with an embedded JPEG must still be found by the scan path.
        // Payload must be large enough to clear find_largest_jpeg's 8-byte minimum.
        let mut data = vec![0xDE, 0xAD, 0xBE, 0xEF];
        let jpeg: Vec<u8> = vec![0xFF, 0xD8, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0xFF, 0xD9];
        data.extend_from_slice(&jpeg);
        let path = write_temp_raf("scan.nef", &data);

        let got = extract_preview_jpeg(&path).expect("should extract via scan");
        assert_eq!(got, jpeg);

        let _ = std::fs::remove_file(&path);
    }
}
