// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::OnceLock;

use crate::error::AppError;

pub struct LutData {
    pub size: usize,
    pub domain_min: [f32; 3],
    pub domain_max: [f32; 3],
    pub table: Vec<f32>,
}

macro_rules! lut_embed {
    ($id:expr, $file:expr) => {
        (
            $id,
            include_bytes!(concat!(env!("OUT_DIR"), "/luts/", $file, ".gz")).as_slice(),
        )
    };
}

static LUT_CACHE: OnceLock<std::collections::HashMap<&'static str, LutData>> = OnceLock::new();

fn compressed_luts() -> Vec<(&'static str, &'static [u8])> {
    vec![
        lut_embed!("acros", "FLog2C_to_ACROS_65grid_V.1.00.cube"),
        lut_embed!("astia", "FLog2C_to_ASTIA_65grid_V.1.00.cube"),
        lut_embed!(
            "classic-chrome",
            "FLog2C_to_CLASSIC-CHROME_65grid_V.1.00.cube"
        ),
        lut_embed!(
            "classic-neg",
            "FLog2C_to_CLASSIC-Neg._65grid_V.1.00.cube"
        ),
        lut_embed!("eterna", "FLog2C_to_ETERNA_65grid_V.1.00.cube"),
        lut_embed!("eterna-bb", "FLog2C_to_ETERNA-BB_65grid_V.1.00.cube"),
        lut_embed!("flog2c-709", "FLog2C_to_FLog2C-709_65grid_V.1.00.cube"),
        lut_embed!(
            "pro-neg-std",
            "FLog2C_to_PRO-Neg.Std_65grid_V.1.00.cube"
        ),
        lut_embed!("provia", "FLog2C_to_PROVIA_65grid_V.1.00.cube"),
        lut_embed!("reala-ace", "FLog2C_to_REALA-ACE_65grid_V.1.00.cube"),
        lut_embed!("velvia", "FLog2C_to_Velvia_65grid_V.1.00.cube"),
    ]
}

pub fn get_lut_data(preset_id: &str) -> Result<&'static LutData, AppError> {
    let cache = LUT_CACHE.get_or_init(|| {
        let mut map = std::collections::HashMap::new();
        for (id, compressed) in compressed_luts() {
            match decompress_and_parse(compressed) {
                Ok(data) => {
                    tracing::info!(
                        "LUT '{}' loaded: {}^3, {} entries",
                        id,
                        data.size,
                        data.table.len() / 3
                    );
                    map.insert(id, data);
                }
                Err(e) => {
                    tracing::error!("Failed to load LUT '{}': {}", id, e);
                }
            }
        }
        map
    });

    cache.get(preset_id).ok_or_else(|| {
        AppError::LutFilterError(format!("LUT data not available for preset: {}", preset_id))
    })
}

fn decompress_and_parse(compressed: &[u8]) -> Result<LutData, AppError> {
    use flate2::read::GzDecoder;
    use std::io::Read;

    let mut decoder = GzDecoder::new(compressed);
    let mut text = String::new();
    decoder
        .read_to_string(&mut text)
        .map_err(|e| AppError::LutFilterError(format!("LUT decompression failed: {}", e)))?;
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
                .map_err(|e| AppError::LutFilterError(format!("Invalid LUT_3D_SIZE: {}", e)))?;
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

        // Skip other header keywords
        if trimmed
            .chars()
            .next()
            .map(|c| c.is_ascii_alphabetic() || c == '_')
            .unwrap_or(false)
        {
            continue;
        }

        // Data line: "R G B"
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() >= 3 {
            let r: f32 = parts[0].parse().map_err(|e| {
                AppError::LutFilterError(format!("Invalid LUT value: {}", e))
            })?;
            let g: f32 = parts[1].parse().map_err(|e| {
                AppError::LutFilterError(format!("Invalid LUT value: {}", e))
            })?;
            let b: f32 = parts[2].parse().map_err(|e| {
                AppError::LutFilterError(format!("Invalid LUT value: {}", e))
            })?;
            table.push(r);
            table.push(g);
            table.push(b);
        }
    }

    let expected = size * size * size * 3;
    if table.len() != expected {
        return Err(AppError::LutFilterError(format!(
            "LUT parse error: expected {} floats, got {}",
            expected,
            table.len()
        )));
    }

    Ok(LutData {
        size,
        domain_min,
        domain_max,
        table,
    })
}

fn parse_three_floats(s: &str, out: &mut [f32; 3]) -> Result<(), AppError> {
    let parts: Vec<&str> = s.trim().split_whitespace().collect();
    if parts.len() < 3 {
        return Err(AppError::LutFilterError(
            "Expected 3 float values".into(),
        ));
    }
    for i in 0..3 {
        out[i] = parts[i].parse().map_err(|e| {
            AppError::LutFilterError(format!("Invalid float: {}", e))
        })?;
    }
    Ok(())
}
