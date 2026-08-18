// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

pub(crate) mod extract;

use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::sync::{Arc, RwLock};

use crate::image_utils::is_raw_file;

const MAX_CACHE_ENTRIES: usize = 50;

/// 缓存总字节预算（128 MB）：预览缓存除条目数上限外再限制总内存占用，
/// 超出预算时按 LRU 逐出最旧条目，避免大图预览把内存吃满。
const MAX_CACHE_BYTES: usize = 128 * 1024 * 1024;

pub fn content_type_for(path: &Path) -> &'static str {
    // RAW files are served as their extracted embedded-JPEG bytes, so the truthful
    // media type is image/jpeg. Returning application/octet-stream here made
    // Chromium/WebView2 refuse to render the <img> (it does not sniff from
    // octet-stream), which broke RAW previews in the Windows preview window.
    if is_raw_file(path) {
        return "image/jpeg";
    }

    let ext = path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "heif" | "hif" | "heic" => "image/heic",
        _ => "application/octet-stream",
    }
}

struct CacheInner {
    data: HashMap<String, Arc<Vec<u8>>>,
    order: VecDeque<String>,
    /// 当前缓存的总字节数（与 data/order 保持同步）
    total_bytes: usize,
}

impl CacheInner {
    /// 按 LRU 逐出最旧条目，直到条目数与总字节数都在预算内。
    fn evict_over_budget(&mut self) {
        while self.order.len() > MAX_CACHE_ENTRIES || self.total_bytes > MAX_CACHE_BYTES {
            let Some(old_key) = self.order.pop_front() else {
                break;
            };
            if let Some(bytes) = self.data.remove(&old_key) {
                self.total_bytes = self.total_bytes.saturating_sub(bytes.len());
            }
        }
    }
}

pub struct ImagePreviewCache {
    inner: RwLock<CacheInner>,
}

impl ImagePreviewCache {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(CacheInner {
                data: HashMap::new(),
                order: VecDeque::new(),
                total_bytes: 0,
            }),
        }
    }

    pub fn get_or_load(&self, path: &Path) -> Result<Arc<Vec<u8>>, String> {
        let key = path.to_string_lossy().to_string();

        {
            let inner = self.inner.read().map_err(|e| e.to_string())?;
            if let Some(bytes) = inner.data.get(&key) {
                return Ok(Arc::clone(bytes));
            }
        }

        let bytes = if is_raw_file(path) {
            Arc::new(extract::extract_preview_jpeg(path)?)
        } else {
            Arc::new(std::fs::read(path).map_err(|e| format!("Failed to read {}: {}", path.display(), e))?)
        };

        {
            let mut inner = self.inner.write().map_err(|e| e.to_string())?;

            if let Some(existing) = inner.data.get(&key) {
                return Ok(Arc::clone(existing));
            }

            inner.total_bytes += bytes.len();
            inner.data.insert(key.clone(), Arc::clone(&bytes));
            inner.order.push_back(key);

            inner.evict_over_budget();
        }

        Ok(bytes)
    }

    pub fn invalidate(&self, path: &Path) {
        let key = path.to_string_lossy().to_string();
        // 与 get_or_load 一致容忍锁中毒：失效操作不应 panic
        let Ok(mut inner) = self.inner.write() else {
            tracing::warn!("Image preview cache lock poisoned, skipping invalidation");
            return;
        };
        if let Some(bytes) = inner.data.remove(&key) {
            inner.total_bytes = inner.total_bytes.saturating_sub(bytes.len());
        }
        inner.order.retain(|k| k != &key);
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
    fn content_type_for_raw_returns_jpeg() {
        // RAW files are served as their extracted embedded JPEG bytes.
        assert_eq!(content_type_for(Path::new("photo.nef")), "image/jpeg");
        assert_eq!(content_type_for(Path::new("photo.cr2")), "image/jpeg");
        assert_eq!(content_type_for(Path::new("photo.raf")), "image/jpeg");
        assert_eq!(content_type_for(Path::new("photo.RAF")), "image/jpeg");
    }

    #[test]
    fn content_type_for_unknown_defaults_to_octet_stream() {
        assert_eq!(content_type_for(Path::new("photo.png")), "application/octet-stream");
        assert_eq!(content_type_for(Path::new("photo.mp4")), "application/octet-stream");
        assert_eq!(content_type_for(Path::new("photo")), "application/octet-stream");
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

        let cache_size = cache.inner.read().unwrap().data.len();
        assert!(
            cache_size <= MAX_CACHE_ENTRIES,
            "Cache should evict, size={}",
            cache_size
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn cache_evicts_to_byte_budget() {
        let mut inner = CacheInner {
            data: HashMap::new(),
            order: VecDeque::new(),
            total_bytes: 0,
        };

        // 3 个各 60MB 的条目（共 180MB > 128MB 预算），应只保留最新的 2 个
        let big: Arc<Vec<u8>> = Arc::new(vec![0u8; 60 * 1024 * 1024]);
        for i in 0..3 {
            let key = format!("test_{}.jpg", i);
            inner.total_bytes += big.len();
            inner.data.insert(key.clone(), Arc::clone(&big));
            inner.order.push_back(key);
        }

        inner.evict_over_budget();

        assert!(
            inner.total_bytes <= MAX_CACHE_BYTES,
            "total bytes should be under budget, got {}",
            inner.total_bytes
        );
        assert_eq!(inner.data.len(), 2, "oldest entry should be evicted");
        assert!(
            !inner.data.contains_key("test_0.jpg"),
            "LRU (oldest) entry should be evicted first"
        );
        // data 与 order 账目一致
        assert_eq!(inner.data.len(), inner.order.len());
        let accounted: usize = inner.data.values().map(|b| b.len()).sum();
        assert_eq!(inner.total_bytes, accounted);
    }

    #[test]
    fn invalidate_removes_matching_key_and_updates_byte_accounting() {
        let dir = std::env::temp_dir().join("cameraftp_test_cache_invalidate");
        std::fs::create_dir_all(&dir).unwrap();

        let cache = ImagePreviewCache::new();
        let keep_path = dir.join("keep.jpg");
        let drop_path = dir.join("drop.jpg");
        let mut f = std::fs::File::create(&keep_path).unwrap();
        f.write_all(&[0xFF, 0xD8, 0x01, 0x02]).unwrap();
        let mut f = std::fs::File::create(&drop_path).unwrap();
        f.write_all(&[0xFF, 0xD8, 0x03, 0x04, 0x05]).unwrap();

        cache.get_or_load(&keep_path).unwrap();
        cache.get_or_load(&drop_path).unwrap();

        let before = cache.inner.read().unwrap().total_bytes;
        assert_eq!(before, 4 + 5);

        cache.invalidate(&drop_path);

        let inner = cache.inner.read().unwrap();
        assert!(!inner.data.contains_key(&drop_path.to_string_lossy().to_string()));
        assert!(inner.data.contains_key(&keep_path.to_string_lossy().to_string()));
        assert_eq!(inner.total_bytes, 4);
        assert_eq!(inner.order.len(), 1);

        drop(inner);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn invalidate_for_unknown_path_is_noop() {
        let cache = ImagePreviewCache::new();
        // 不应 panic，也不改变任何状态
        cache.invalidate(Path::new("/nonexistent/photo.jpg"));
        let inner = cache.inner.read().unwrap();
        assert!(inner.data.is_empty());
        assert!(inner.order.is_empty());
        assert_eq!(inner.total_bytes, 0);
    }
}
