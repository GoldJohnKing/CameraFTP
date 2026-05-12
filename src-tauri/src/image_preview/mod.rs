// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

pub(crate) mod extract;

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};

use crate::image_utils::is_raw_file;

/// Get MIME type based on file extension.
pub fn content_type_for(path: &Path) -> &'static str {
    let ext = path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "heif" | "hif" | "heic" => "image/heic",
        _ => "image/jpeg",
    }
}

/// In-memory cache for image preview bytes.
pub struct ImagePreviewCache {
    cache: RwLock<HashMap<String, Arc<Vec<u8>>>>,
}

impl ImagePreviewCache {
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
        }
    }

    /// Get cached image bytes, or extract/read from file and cache the result.
    pub fn get_or_load(&self, path: &Path) -> Result<Arc<Vec<u8>>, String> {
        let key = path.to_string_lossy().to_string();

        // Fast path: check cache with read lock
        {
            let cache = self.cache.read().map_err(|e| e.to_string())?;
            if let Some(bytes) = cache.get(&key) {
                return Ok(Arc::clone(bytes));
            }
        }

        // Slow path: load/extract
        let bytes = if is_raw_file(path) {
            Arc::new(extract::extract_preview_jpeg(path)?)
        } else {
            Arc::new(std::fs::read(path).map_err(|e| format!("Failed to read {}: {}", path.display(), e))?)
        };

        // Store in cache
        {
            let mut cache = self.cache.write().map_err(|e| e.to_string())?;
            cache.insert(key, Arc::clone(&bytes));
        }

        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn content_type_for_jpeg_extensions() {
        assert_eq!(content_type_for(Path::new("photo.jpg")), "image/jpeg");
        assert_eq!(content_type_for(Path::new("photo.jpeg")), "image/jpeg");
        assert_eq!(content_type_for(Path::new("photo.JPG")), "image/jpeg");
    }

    #[test]
    fn content_type_for_heif_extensions() {
        assert_eq!(content_type_for(Path::new("photo.heif")), "image/heic");
        assert_eq!(content_type_for(Path::new("photo.hif")), "image/heic");
        assert_eq!(content_type_for(Path::new("photo.heic")), "image/heic");
    }

    #[test]
    fn content_type_for_unknown_defaults_to_jpeg() {
        assert_eq!(content_type_for(Path::new("photo.nef")), "image/jpeg");
        assert_eq!(content_type_for(Path::new("photo.cr2")), "image/jpeg");
        assert_eq!(content_type_for(Path::new("photo.png")), "image/jpeg");
        assert_eq!(content_type_for(Path::new("photo")), "image/jpeg");
    }
}
