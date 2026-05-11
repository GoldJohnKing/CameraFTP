// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ColorGradingPreset {
    pub id: String,
    pub display_name: String,
    pub log_space: String,
    pub cube_filename: String,
}

static COLOR_GRADING_PRESETS: OnceLock<Vec<ColorGradingPreset>> = OnceLock::new();

/// Default preset ID used when no user preference is stored.
pub const DEFAULT_PRESET_ID: &str = "fujifilm-provia";

pub fn all_presets() -> &'static [ColorGradingPreset] {
    COLOR_GRADING_PRESETS.get_or_init(|| vec![
        ColorGradingPreset { id: "arri-alexa-classic-709".into(), display_name: "ARRI ALEXA Classic 709".into(), log_space: "V-Log".into(), cube_filename: "ARRI_ALEXA_Classic-709_VLog.cube".into() },
        ColorGradingPreset { id: "fujifilm-acros".into(), display_name: "Fujifilm ACROS".into(), log_space: "V-Log".into(), cube_filename: "Fujifilm_ACROS_VLog.cube".into() },
        ColorGradingPreset { id: "fujifilm-astia".into(), display_name: "Fujifilm ASTIA".into(), log_space: "V-Log".into(), cube_filename: "Fujifilm_ASTIA_VLog.cube".into() },
        ColorGradingPreset { id: "fujifilm-classic-chrome".into(), display_name: "Fujifilm CLASSIC CHROME".into(), log_space: "V-Log".into(), cube_filename: "Fujifilm_CLASSIC-CHROME_VLog.cube".into() },
        ColorGradingPreset { id: "fujifilm-classic-neg".into(), display_name: "Fujifilm CLASSIC Neg".into(), log_space: "V-Log".into(), cube_filename: "Fujifilm_CLASSIC-Neg_VLog.cube".into() },
        ColorGradingPreset { id: "fujifilm-eterna-3513di".into(), display_name: "Fujifilm ETERNA 3513DI".into(), log_space: "V-Log".into(), cube_filename: "Fujifilm_ETERNA-3513DI_VLog.cube".into() },
        ColorGradingPreset { id: "fujifilm-eterna-bb".into(), display_name: "Fujifilm ETERNA BB".into(), log_space: "V-Log".into(), cube_filename: "Fujifilm_ETERNA-BB_VLog.cube".into() },
        ColorGradingPreset { id: "fujifilm-eterna".into(), display_name: "Fujifilm ETERNA".into(), log_space: "V-Log".into(), cube_filename: "Fujifilm_ETERNA_VLog.cube".into() },
        ColorGradingPreset { id: "fujifilm-pro-neg-std".into(), display_name: "Fujifilm PRO Neg. Std".into(), log_space: "V-Log".into(), cube_filename: "Fujifilm_PRO-Neg.-Std_VLog.cube".into() },
        ColorGradingPreset { id: "fujifilm-provia".into(), display_name: "Fujifilm PROVIA".into(), log_space: "V-Log".into(), cube_filename: "Fujifilm_PROVIA_VLog.cube".into() },
        ColorGradingPreset { id: "fujifilm-reala-ace".into(), display_name: "Fujifilm REALA ACE".into(), log_space: "V-Log".into(), cube_filename: "Fujifilm_REALA-ACE_VLog.cube".into() },
        ColorGradingPreset { id: "fujifilm-velvia".into(), display_name: "Fujifilm Velvia".into(), log_space: "V-Log".into(), cube_filename: "Fujifilm_Velvia_VLog.cube".into() },
        ColorGradingPreset { id: "kodak-vision-2383".into(), display_name: "Kodak VISION 2383".into(), log_space: "V-Log".into(), cube_filename: "Kodak_VISION-2383_VLog.cube".into() },
        ColorGradingPreset { id: "leica-classic".into(), display_name: "Leica Classic".into(), log_space: "V-Log".into(), cube_filename: "Leica_Classic_VLog.cube".into() },
        ColorGradingPreset { id: "leica-natural".into(), display_name: "Leica Natural".into(), log_space: "V-Log".into(), cube_filename: "Leica_Natural_VLog.cube".into() },
        ColorGradingPreset { id: "red-achromic".into(), display_name: "RED Achromic".into(), log_space: "V-Log".into(), cube_filename: "RED_Achromic_VLog.cube".into() },
        ColorGradingPreset { id: "red-filmbias-bb".into(), display_name: "RED FilmBias BB".into(), log_space: "V-Log".into(), cube_filename: "RED_FilmBias-BB_VLog.cube".into() },
        ColorGradingPreset { id: "red-filmbias-offset".into(), display_name: "RED FilmBias Offset".into(), log_space: "V-Log".into(), cube_filename: "RED_FilmBias-Offset_VLog.cube".into() },
        ColorGradingPreset { id: "red-filmbias".into(), display_name: "RED FilmBias".into(), log_space: "V-Log".into(), cube_filename: "RED_FilmBias_VLog.cube".into() },
        ColorGradingPreset { id: "red-rec-709".into(), display_name: "RED Rec.709".into(), log_space: "V-Log".into(), cube_filename: "RED_Rec.709_VLog.cube".into() },
    ])
}

pub fn find_preset(id: &str) -> Option<&'static ColorGradingPreset> {
    all_presets().iter().find(|p| p.id == id)
}

/// Supported automatic exposure metering modes (value, display name).
pub const METERING_MODES: &[(&str, &str)] = &[
    ("highlight-safe", "高光保护"),
    ("matrix", "矩阵测光"),
    ("center-weighted", "中央重点测光"),
    ("average", "平均测光"),
    ("hybrid", "混合测光"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_existing_preset() {
        let p = find_preset("fujifilm-classic-neg").unwrap();
        assert_eq!(p.display_name, "Fujifilm CLASSIC Neg");
        assert_eq!(p.log_space, "V-Log");
    }

    #[test]
    fn find_nonexistent_returns_none() {
        assert!(find_preset("nonexistent").is_none());
    }

    #[test]
    fn all_presets_have_unique_ids() {
        let ids: Vec<&str> = all_presets().iter().map(|p| p.id.as_str()).collect();
        let unique: std::collections::HashSet<&str> = ids.iter().copied().collect();
        assert_eq!(ids.len(), unique.len());
    }
}
