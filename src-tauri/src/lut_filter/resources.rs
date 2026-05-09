// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::PathBuf;
use std::sync::OnceLock;

use crate::error::AppError;

const RESOURCE_VERSION: &str = "1";

pub struct ResourcePaths {
    pub lensfun_db_dir: PathBuf,
    pub lut_presets_dir: PathBuf,
}

static GLOBAL_RESOURCES: OnceLock<ResourcePaths> = OnceLock::new();

pub fn ensure_resources(
    app_data_dir: &std::path::Path,
) -> Result<&'static ResourcePaths, AppError> {
    if let Some(res) = GLOBAL_RESOURCES.get() {
        return Ok(res);
    }

    let lut_dir = app_data_dir.join("lut_presets");
    let lensfun_dir = app_data_dir.join("lensfun_db");
    let version_file = app_data_dir.join(".lut_filter_resources_version");

    let needs_extraction = !version_file.exists()
        || std::fs::read_to_string(&version_file).unwrap_or_default() != RESOURCE_VERSION;

    if needs_extraction {
        extract_resources(app_data_dir, &lut_dir, &lensfun_dir)?;
        std::fs::write(&version_file, RESOURCE_VERSION).map_err(|e| {
            AppError::LutFilterError(format!("Failed to write version marker: {}", e))
        })?;
    }

    tracing::info!(
        "LUT filter resources ready: luts={:?}, lensfun={:?}",
        lut_dir,
        lensfun_dir
    );
    let _ = GLOBAL_RESOURCES.set(ResourcePaths {
        lensfun_db_dir: lensfun_dir,
        lut_presets_dir: lut_dir,
    });
    Ok(GLOBAL_RESOURCES.get().unwrap())
}

pub fn get_resources() -> Result<&'static ResourcePaths, AppError> {
    GLOBAL_RESOURCES.get().ok_or_else(|| {
        AppError::LutFilterError(
            "Resources not initialized. Call ensure_resources() first.".into(),
        )
    })
}

fn extract_resources(
    app_data_dir: &std::path::Path,
    lut_dir: &std::path::Path,
    lensfun_dir: &std::path::Path,
) -> Result<(), AppError> {
    std::fs::create_dir_all(app_data_dir).map_err(|e| {
        AppError::LutFilterError(format!("Failed to create app data dir: {}", e))
    })?;
    std::fs::create_dir_all(lut_dir)
        .map_err(|e| AppError::LutFilterError(format!("Failed to create lut dir: {}", e)))?;
    std::fs::create_dir_all(lensfun_dir).map_err(|e| {
        AppError::LutFilterError(format!("Failed to create lensfun dir: {}", e))
    })?;

    let exe_dir = std::env::current_exe()
        .map_err(|e| AppError::LutFilterError(format!("Failed to get exe path: {}", e)))?
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_default();

    let lut_sources = [
        exe_dir.join("resources").join("luts"),
        PathBuf::from("../../F-Log2C_LUT"),
    ];

    let lensfun_sources = [
        exe_dir.join("resources").join("lensfun_db"),
        PathBuf::from("../lib/lensfun/data/db"),
    ];

    copy_files(&lut_sources, lut_dir, "*.cube")?;
    copy_files(&lensfun_sources, lensfun_dir, "*.xml")?;

    Ok(())
}

fn copy_files(
    source_dirs: &[PathBuf],
    target_dir: &std::path::Path,
    pattern: &str,
) -> Result<usize, AppError> {
    let ext_match = if pattern.starts_with("*.") {
        &pattern[2..]
    } else {
        pattern
    };

    for source_dir in source_dirs {
        if !source_dir.exists() {
            continue;
        }
        let mut count = 0usize;
        let entries = std::fs::read_dir(source_dir).map_err(|e| {
            AppError::LutFilterError(format!("Failed to read {:?}: {}", source_dir, e))
        })?;

        for entry in entries {
            let entry =
                entry.map_err(|e| AppError::LutFilterError(format!("Dir entry error: {}", e)))?;
            let path = entry.path();
            if let Some(ext) = path.extension() {
                if ext == ext_match {
                    let file_name = path.file_name().unwrap_or_default();
                    let dest = target_dir.join(file_name);
                    std::fs::copy(&path, &dest).map_err(|e| {
                        AppError::LutFilterError(format!("Failed to copy {:?}: {}", path, e))
                    })?;
                    count += 1;
                }
            }
        }
        if count > 0 {
            tracing::info!(
                "Copied {} files from {:?} to {:?}",
                count,
                source_dir,
                target_dir
            );
            return Ok(count);
        }
    }
    tracing::warn!(
        "No files matching '{}' found in any source directory",
        pattern
    );
    Ok(0)
}
