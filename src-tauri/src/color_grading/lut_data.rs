// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::{Arc, LazyLock};

use dashmap::DashMap;

use crate::error::AppError;

pub struct LutData {
    pub size: usize,
    pub domain_min: [f32; 3],
    pub domain_max: [f32; 3],
    pub table: Arc<Vec<f32>>,
}

static LUT_ZIP: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/luts.zip"));

static LUT_CACHE: LazyLock<DashMap<String, Arc<LutData>>> = LazyLock::new(DashMap::new);

pub fn get_lut_data(preset_id: &str) -> Result<Arc<LutData>, AppError> {
    if let Some(entry) = LUT_CACHE.get(preset_id) {
        return Ok(Arc::clone(entry.value()));
    }

    let preset = super::presets::find_preset(preset_id).ok_or_else(|| {
        AppError::ColorGradingError(format!("Unknown color grading preset: {}", preset_id))
    })?;

    let lut_data = Arc::new(extract_and_parse(&preset.cube_filename)?);

    tracing::info!(
        "LUT '{}' loaded: {}^3, {} entries",
        preset_id,
        lut_data.size,
        lut_data.table.len() / 3
    );

    LUT_CACHE.insert(preset_id.to_string(), Arc::clone(&lut_data));

    Ok(lut_data)
}

fn extract_and_parse(cube_filename: &str) -> Result<LutData, AppError> {
    let reader = std::io::Cursor::new(LUT_ZIP);
    let mut archive = zip::ZipArchive::new(reader).map_err(|e| {
        AppError::ColorGradingError(format!("Failed to open LUT ZIP archive: {}", e))
    })?;

    let mut file = archive.by_name(cube_filename).map_err(|e| {
        AppError::ColorGradingError(format!(
            "LUT '{}' not found in archive: {}",
            cube_filename, e
        ))
    })?;

    let mut text = String::new();
    std::io::Read::read_to_string(&mut file, &mut text).map_err(|e| {
        AppError::ColorGradingError(format!("Failed to read LUT entry '{}': {}", cube_filename, e))
    })?;

    parse_cube_text(&text)
}

fn parse_cube_text(text: &str) -> Result<LutData, AppError> {
    let mut size: usize = 0;
    let mut domain_min = [0.0f32; 3];
    let mut domain_max = [1.0f32; 3];
    let mut table = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("LUT_3D_SIZE") {
            size = rest
                .trim()
                .parse::<usize>()
                .map_err(|e| AppError::ColorGradingError(format!("Invalid LUT_3D_SIZE: {}", e)))?;
            table.reserve(size * size * size * 3);
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("DOMAIN_MIN") {
            parse_three_floats(rest, &mut domain_min)?;
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("DOMAIN_MAX") {
            parse_three_floats(rest, &mut domain_max)?;
            continue;
        }

        if trimmed
            .chars()
            .next()
            .map(|c| c.is_ascii_alphabetic() || c == '_')
            .unwrap_or(false)
        {
            continue;
        }

        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() >= 3 {
            let r: f32 = parts[0].parse().map_err(|e| {
                AppError::ColorGradingError(format!("Invalid LUT value: {}", e))
            })?;
            let g: f32 = parts[1].parse().map_err(|e| {
                AppError::ColorGradingError(format!("Invalid LUT value: {}", e))
            })?;
            let b: f32 = parts[2].parse().map_err(|e| {
                AppError::ColorGradingError(format!("Invalid LUT value: {}", e))
            })?;
            table.push(r);
            table.push(g);
            table.push(b);
        }
    }

    let expected = size * size * size * 3;
    if table.len() != expected {
        return Err(AppError::ColorGradingError(format!(
            "LUT parse error: expected {} floats, got {}",
            expected,
            table.len()
        )));
    }

    Ok(LutData {
        size,
        domain_min,
        domain_max,
        table: Arc::new(table),
    })
}

fn parse_three_floats(s: &str, out: &mut [f32; 3]) -> Result<(), AppError> {
    let mut parts = s.trim().split_whitespace();
    for i in 0..3 {
        out[i] = parts
            .next()
            .ok_or_else(|| AppError::ColorGradingError("Expected 3 float values".into()))?
            .parse()
            .map_err(|e| AppError::ColorGradingError(format!("Invalid float: {}", e)))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_2x2x2_cube() -> String {
        [
            "# Minimal 2x2x2 LUT",
            "TITLE \"test\"",
            "",
            "LUT_3D_SIZE 2",
            "",
            "0.0 0.0 0.0",
            "1.0 0.0 0.0",
            "0.0 1.0 0.0",
            "1.0 1.0 0.0",
            "0.0 0.0 1.0",
            "1.0 0.0 1.0",
            "0.0 1.0 1.0",
            "1.0 1.0 1.0",
        ]
        .join("\n")
    }

    #[test]
    fn parse_cube_minimal_valid() {
        let lut = parse_cube_text(&minimal_2x2x2_cube()).unwrap();
        assert_eq!(lut.size, 2);
        assert_eq!(lut.domain_min, [0.0, 0.0, 0.0]);
        assert_eq!(lut.domain_max, [1.0, 1.0, 1.0]);
        assert_eq!(lut.table.len(), 2 * 2 * 2 * 3);

        // First triplet: 0.0 0.0 0.0
        assert_eq!(lut.table[0], 0.0);
        assert_eq!(lut.table[1], 0.0);
        assert_eq!(lut.table[2], 0.0);

        // Second triplet: 1.0 0.0 0.0
        assert_eq!(lut.table[3], 1.0);
        assert_eq!(lut.table[4], 0.0);
        assert_eq!(lut.table[5], 0.0);

        // Last triplet: 1.0 1.0 1.0
        assert_eq!(lut.table[21], 1.0);
        assert_eq!(lut.table[22], 1.0);
        assert_eq!(lut.table[23], 1.0);
    }

    #[test]
    fn parse_cube_with_domain_directives() {
        let text = [
            "LUT_3D_SIZE 2",
            "DOMAIN_MIN 0.1 0.2 0.3",
            "DOMAIN_MAX 0.9 0.8 0.7",
            "0.0 0.0 0.0",
            "1.0 0.0 0.0",
            "0.0 1.0 0.0",
            "1.0 1.0 0.0",
            "0.0 0.0 1.0",
            "1.0 0.0 1.0",
            "0.0 1.0 1.0",
            "1.0 1.0 1.0",
        ]
        .join("\n");

        let lut = parse_cube_text(&text).unwrap();
        assert_eq!(lut.domain_min, [0.1, 0.2, 0.3]);
        assert_eq!(lut.domain_max, [0.9, 0.8, 0.7]);
    }

    #[test]
    fn parse_cube_skips_comments_and_keywords() {
        let text = [
            "# This is a comment",
            "TITLE \"My LUT\"",
            "LUT_3D_SIZE 2",
            "# Another comment",
            "CREATED_BY \"test\"",
            "",
            "0.0 0.0 0.0",
            "1.0 0.0 0.0",
            "0.0 1.0 0.0",
            "1.0 1.0 0.0",
            "0.0 0.0 1.0",
            "1.0 0.0 1.0",
            "0.0 1.0 1.0",
            "1.0 1.0 1.0",
        ]
        .join("\n");

        let lut = parse_cube_text(&text).unwrap();
        assert_eq!(lut.size, 2);
        assert_eq!(lut.table.len(), 24);
    }

    #[test]
    fn parse_cube_wrong_row_count_returns_error() {
        let text = [
            "LUT_3D_SIZE 2",
            "0.0 0.0 0.0",
            "1.0 0.0 0.0",
            "0.0 1.0 0.0",
            // Only 3 of 8 rows provided
        ]
        .join("\n");

        let result = parse_cube_text(&text);
        assert!(result.is_err(), "Should fail with wrong row count");
        if let Err(AppError::ColorGradingError(msg)) = result {
            assert!(
                msg.contains("expected") && msg.contains("got"),
                "Error should mention expected vs got counts: {}",
                msg
            );
        } else {
            panic!("Expected ColorGradingError");
        }
    }

    #[test]
    fn parse_three_floats_valid() {
        let mut out = [0.0f32; 3];
        parse_three_floats("0.1 0.2 0.3", &mut out).unwrap();
        assert!((out[0] - 0.1).abs() < f32::EPSILON);
        assert!((out[1] - 0.2).abs() < f32::EPSILON);
        assert!((out[2] - 0.3).abs() < f32::EPSILON);
    }

    #[test]
    fn parse_three_floats_too_few_values() {
        let mut out = [0.0f32; 3];
        if let Err(AppError::ColorGradingError(msg)) =
            parse_three_floats("0.1 0.2", &mut out)
        {
            assert!(
                msg.contains("Expected 3 float values"),
                "Should report missing values: {}",
                msg
            );
        } else {
            panic!("Expected ColorGradingError");
        }
    }

    #[test]
    fn parse_three_floats_non_numeric() {
        let mut out = [0.0f32; 3];
        if let Err(AppError::ColorGradingError(msg)) =
            parse_three_floats("abc def ghi", &mut out)
        {
            assert!(
                msg.contains("Invalid float"),
                "Should report parse failure: {}",
                msg
            );
        } else {
            panic!("Expected ColorGradingError");
        }
    }
}
