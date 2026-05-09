// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct PresetLut {
    pub id: String,
    pub display_name: String,
    pub log_space: String,
    pub cube_filename: String,
}

static PRESET_LUTS: OnceLock<Vec<PresetLut>> = OnceLock::new();

pub fn all_presets() -> &'static [PresetLut] {
    PRESET_LUTS.get_or_init(|| vec![
        PresetLut { id: "acros".into(), display_name: "ACROS".into(), log_space: "F-Log2C".into(), cube_filename: "FLog2C_to_ACROS_65grid_V.1.00.cube".into() },
        PresetLut { id: "astia".into(), display_name: "Astia".into(), log_space: "F-Log2C".into(), cube_filename: "FLog2C_to_ASTIA_65grid_V.1.00.cube".into() },
        PresetLut { id: "classic-chrome".into(), display_name: "Classic Chrome".into(), log_space: "F-Log2C".into(), cube_filename: "FLog2C_to_CLASSIC-CHROME_65grid_V.1.00.cube".into() },
        PresetLut { id: "classic-neg".into(), display_name: "Classic Neg".into(), log_space: "F-Log2C".into(), cube_filename: "FLog2C_to_CLASSIC-Neg._65grid_V.1.00.cube".into() },
        PresetLut { id: "eterna".into(), display_name: "ETERNA".into(), log_space: "F-Log2C".into(), cube_filename: "FLog2C_to_ETERNA_65grid_V.1.00.cube".into() },
        PresetLut { id: "eterna-bb".into(), display_name: "ETERNA Bleach Bypass".into(), log_space: "F-Log2C".into(), cube_filename: "FLog2C_to_ETERNA-BB_65grid_V.1.00.cube".into() },
        PresetLut { id: "pro-neg-std".into(), display_name: "PRO Neg. Std".into(), log_space: "F-Log2C".into(), cube_filename: "FLog2C_to_PRO-Neg.Std_65grid_V.1.00.cube".into() },
        PresetLut { id: "provia".into(), display_name: "Provia".into(), log_space: "F-Log2C".into(), cube_filename: "FLog2C_to_PROVIA_65grid_V.1.00.cube".into() },
        PresetLut { id: "reala-ace".into(), display_name: "REALA ACE".into(), log_space: "F-Log2C".into(), cube_filename: "FLog2C_to_REALA-ACE_65grid_V.1.00.cube".into() },
        PresetLut { id: "velvia".into(), display_name: "Velvia".into(), log_space: "F-Log2C".into(), cube_filename: "FLog2C_to_Velvia_65grid_V.1.00.cube".into() },
        PresetLut { id: "flog2c-709".into(), display_name: "F-Log2C → Rec.709".into(), log_space: "F-Log2C".into(), cube_filename: "FLog2C_to_FLog2C-709_65grid_V.1.00.cube".into() },
    ])
}

pub fn find_preset(id: &str) -> Option<&'static PresetLut> {
    all_presets().iter().find(|p| p.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_existing_preset() {
        let p = find_preset("classic-neg").unwrap();
        assert_eq!(p.display_name, "Classic Neg");
        assert_eq!(p.log_space, "F-Log2C");
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
