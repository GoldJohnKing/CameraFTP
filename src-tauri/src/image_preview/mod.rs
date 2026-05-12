// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

pub(crate) mod extract;

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

use crate::image_utils::is_raw_file;

const MAX_CACHE_ENTRIES: usize = 50;

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

pub struct ImagePreviewCache {
    cache: RwLock<HashMap<String, Arc<Vec<u8>>>>,
    insertion_order: RwLock<Vec<String>>,
    size: AtomicUsize,
}

impl ImagePreviewCache {
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            insertion_order: RwLock::new(Vec::new()),
            size: AtomicUsize::new(0),
        }
    }

    pub fn get_or_load(&self, path: &Path) -> Result<Arc<Vec<u8>>, String> {
        let key = path.to_string_lossy().to_string();

        {
            let cache = self.cache.read().map_err(|e| e.to_string())?;
            if let Some(bytes) = cache.get(&key) {
                return Ok(Arc::clone(bytes));
            }
        }

        let bytes = if is_raw_file(path) {
            Arc::new(extract::extract_preview_jpeg(path)?)
        } else {
            Arc::new(std::fs::read(path).map_err(|e| format!("Failed to read {}: {}", path.display(), e))?)
        };

        {
            let mut cache = self.cache.write().map_err(|e| e.to_string())?;
            let mut order = self.insertion_order.write().map_err(|e| e.to_string())?;

            // Double-check under write lock: another thread may have loaded this key
            if let Some(existing) = cache.get(&key) {
                return Ok(Arc::clone(existing));
            }

            cache.insert(key.clone(), Arc::clone(&bytes));
            order.push(key);

            while order.len() > MAX_CACHE_ENTRIES {
                let old_key = order.remove(0);
                cache.remove(&old_key);
                self.size.fetch_sub(1, Ordering::Relaxed);
            }
        }

        self.size.fetch_add(1, Ordering::Relaxed);

        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
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

    #[test]
    fn cache_returns_same_instance_for_same_path() {
        let dir = std::env::temp_dir().join("cameraftp_test_cache_instance");
        std::fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("test.jpg");
        let mut f = std::fs::File::create(&file_path).unwrap();
        f.write_all(&[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x02, 0x00, 0x00]).unwrap();

        let cache = ImagePreviewCache::new();
        let result1 = cache.get_or_load(&file_path).unwrap();
        let result2 = cache.get_or_load(&file_path).unwrap();
        assert!(Arc::ptr_eq(&result1, &result2));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn cache_evicts_old_entries() {
        let dir = std::env::temp_dir().join("cameraftp_test_cache_eviction");
        std::fs::create_dir_all(&dir).unwrap();

        let cache = ImagePreviewCache::new();

        for i in 0..60 {
            let file_path = dir.join(format!("test_{}.jpg", i));
            let mut f = std::fs::File::create(&file_path).unwrap();
            f.write_all(&[0xFF, 0xD8, 0x00, 0x00]).unwrap();
            cache.get_or_load(&file_path).unwrap();
        }

        let cache_size = cache.size.load(Ordering::Relaxed);
        assert!(
            cache_size <= MAX_CACHE_ENTRIES,
            "Cache should evict, size={}",
            cache_size
        );

        std::fs::remove_dir_all(&dir).ok();
    }
}
