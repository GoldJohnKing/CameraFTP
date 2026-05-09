// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Embedded Lensfun DB — XML files are compressed at compile time via build.rs,
//! embedded into the binary with `include_bytes!()`, and extracted to disk at
//! runtime so the C++ Lensfun library can load them.

use std::path::Path;
use std::sync::OnceLock;

use crate::error::AppError;

// Include the auto-generated manifest (LENSFUN_DB_HASH + LENSFUN_DB_FILES)
include!(concat!(env!("OUT_DIR"), "/lensfun_db_manifest.rs"));

pub struct LensfunDbPaths {
    pub db_dir: std::path::PathBuf,
}

static GLOBAL_DB: OnceLock<LensfunDbPaths> = OnceLock::new();

/// Ensure the Lensfun DB is extracted to `{app_data_dir}/lensfun_db/`.
///
/// Uses a content hash to detect changes — only re-extracts when the embedded
/// data differs from what's on disk.
pub fn ensure_db(app_data_dir: &Path) -> Result<(), AppError> {
    if GLOBAL_DB.get().is_some() {
        return Ok(());
    }

    // No embedded DB files — skip silently
    if LENSFUN_DB_FILES.is_empty() {
        tracing::info!("No embedded Lensfun DB files — lens correction unavailable");
        return Ok(());
    }

    let db_dir = app_data_dir.join("lensfun_db");
    let hash_file = app_data_dir.join(".lensfun_db_hash");

    let needs_extraction = match std::fs::read_to_string(&hash_file) {
        Ok(stored) => stored != LENSFUN_DB_HASH,
        Err(_) => true,
    };

    if needs_extraction {
        extract_db_files(&db_dir)?;
        std::fs::write(&hash_file, LENSFUN_DB_HASH).map_err(|e| {
            AppError::LutFilterError(format!("Failed to write lensfun DB hash: {}", e))
        })?;
        tracing::info!(
            "Lensfun DB extracted ({} files, hash={})",
            LENSFUN_DB_FILES.len(),
            LENSFUN_DB_HASH
        );
    } else {
        tracing::info!(
            "Lensfun DB up-to-date ({} files, hash={})",
            LENSFUN_DB_FILES.len(),
            LENSFUN_DB_HASH
        );
    }

    let _ = GLOBAL_DB.set(LensfunDbPaths { db_dir });
    Ok(())
}

pub fn get_db() -> Result<&'static LensfunDbPaths, AppError> {
    GLOBAL_DB.get().ok_or_else(|| {
        AppError::LutFilterError(
            "Lensfun DB not initialized. Call ensure_db() first.".into(),
        )
    })
}

fn extract_db_files(db_dir: &Path) -> Result<(), AppError> {
    use flate2::read::GzDecoder;
    use std::io::Read;

    std::fs::create_dir_all(db_dir).map_err(|e| {
        AppError::LutFilterError(format!("Failed to create lensfun DB dir: {}", e))
    })?;

    // Clear existing files to avoid stale data from a previous version
    if db_dir.exists() {
        if let Err(e) = std::fs::remove_dir_all(db_dir) {
            tracing::warn!("Failed to clear old lensfun DB: {}", e);
        }
        std::fs::create_dir_all(db_dir).map_err(|e| {
            AppError::LutFilterError(format!("Failed to recreate lensfun DB dir: {}", e))
        })?;
    }

    for &(filename, compressed) in LENSFUN_DB_FILES {
        let output_path = db_dir.join(filename);
        let mut decoder = GzDecoder::new(compressed);
        let mut xml_data = Vec::new();
        decoder.read_to_end(&mut xml_data).map_err(|e| {
            AppError::LutFilterError(format!(
                "Failed to decompress lensfun DB file '{}': {}",
                filename, e
            ))
        })?;
        std::fs::write(&output_path, &xml_data).map_err(|e| {
            AppError::LutFilterError(format!(
                "Failed to write lensfun DB file '{}': {}",
                filename, e
            ))
        })?;
    }

    Ok(())
}
