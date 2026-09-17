// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

pub(crate) mod extract;

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
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

/// 校验 image-preview 请求的路径是否位于保存目录之内。
///
/// 返回:
/// - `Ok(Some(canonical))`: 路径存在、是文件、且位于 `save_root` 之下，
///   返回规范化后的绝对路径（解析 `..`、符号链接与 Windows `\\?\` 前缀）
/// - `Ok(None)`: 路径越界或不是文件，调用方应回 404（与 `Err` 分支统一，
///   避免 403 构成存在性 oracle——否则调用方可区分"路径存在但越界"与
///   "路径不存在"）
/// - `Err(e)`: 路径不存在/无法访问，调用方应回 404
///
/// 两端各自 `canonicalize` 后用 `starts_with` 判断包含关系，可抵御目录穿越。
pub(crate) fn validate_preview_path(
    requested: &Path,
    save_root: &Path,
) -> std::io::Result<Option<PathBuf>> {
    let canonical_requested = requested.canonicalize()?;
    let canonical_root = save_root.canonicalize()?;

    if !canonical_requested.starts_with(&canonical_root) {
        return Ok(None);
    }
    if !canonical_requested.is_file() {
        return Ok(None);
    }
    Ok(Some(canonical_requested))
}

/// 统一缓存键：路径分隔符归一（反斜杠 → 正斜杠）。
///
/// 索引有两个 raw 路径生产者：scan/watcher 走 `PathBuf`（Windows 上通常
/// 带反斜杠），FTP 事件路径是正斜杠字符串拼出来的 `save_path.join(&path)`。
/// 字符串键若不做归一，同一路径会因拼写不同而 miss/失效变 no-op，因此
/// `get_or_load` 与 `invalidate` 双边必须同用本函数。
/// 不可用 `canonicalize`：invalidate 常在文件已删除后调用（canonicalize
/// 对已删路径失败），且 canonical 形态（Windows `\\?\` verbatim）与请求
/// 路径永不匹配。
fn cache_key(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
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

impl Default for ImagePreviewCache {
    fn default() -> Self {
        Self::new()
    }
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
        // 键契约：缓存键 = 调用方传入的原始路径字符串经分隔符归一（见
        // cache_key）。invalidate 也按同一规则归一——见 lib.rs scheme
        // handler 注释与 cache_key doc。
        let key = cache_key(path);

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
        // 与 get_or_load 同规则归一（cache_key），保证跨生产者命中
        let key = cache_key(path);
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
        // 键已归一（cache_key）：内部断言也用归一键，避免空洞通过
        assert!(!inner.data.contains_key(&cache_key(&drop_path)));
        assert!(inner.data.contains_key(&cache_key(&keep_path)));
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

    #[test]
    fn validate_preview_path_accepts_file_inside_save_root() {
        let dir = std::env::temp_dir().join("cameraftp_test_preview_validate");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("nested")).unwrap();
        let file = dir.join("nested/photo.jpg");
        std::fs::write(&file, b"jpeg-bytes").unwrap();

        let resolved = validate_preview_path(&file, &dir).expect("canonicalize should succeed");
        assert!(resolved.is_some(), "file inside save_root must be accepted");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn validate_preview_path_rejects_escape_via_dotdot() {
        let base = std::env::temp_dir().join("cameraftp_test_preview_escape");
        let _ = std::fs::remove_dir_all(&base);
        let root = base.join("root");
        let outside = base.join("secret.jpg");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(&outside, b"secret").unwrap();

        // 请求 root/../secret.jpg — canonicalize 后位于 root 之外
        let requested = root.join("../secret.jpg");
        let resolved = validate_preview_path(&requested, &root).expect("canonicalize should succeed");
        assert!(resolved.is_none(), "path escaping save_root must be rejected");

        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn validate_preview_path_rejects_directory() {
        let base = std::env::temp_dir().join("cameraftp_test_preview_dir");
        let _ = std::fs::remove_dir_all(&base);
        let sub = base.join("sub");
        std::fs::create_dir_all(&sub).unwrap();

        let resolved =
            validate_preview_path(&sub, &base).expect("canonicalize should succeed");
        assert!(resolved.is_none(), "directory targets must be rejected");

        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn validate_preview_path_errors_for_missing_file() {
        let missing = std::env::temp_dir().join("cameraftp_test_preview_missing/none.jpg");
        let result = validate_preview_path(&missing, &std::env::temp_dir());
        assert!(result.is_err(), "missing path should surface an io error");
    }

    // Windows 创建 symlink 需要开发者模式/管理员权限，故仅在 unix 验证
    // （canonicalize 语义一致：都解析符号链接后再判包含关系）
    #[cfg(unix)]
    #[test]
    fn validate_preview_path_rejects_symlink_escaping_save_root() {
        use std::os::unix::fs::symlink;

        let base = std::env::temp_dir().join("cameraftp_test_preview_symlink");
        let _ = std::fs::remove_dir_all(&base);
        let root = base.join("root");
        std::fs::create_dir_all(&root).unwrap();
        let outside = base.join("outside.jpg");
        std::fs::write(&outside, b"secret").unwrap();

        // 根内 symlink 指向根外文件：canonicalize 解析后位于 root 之外 → Ok(None)
        let link = root.join("link.jpg");
        symlink(&outside, &link).unwrap();

        let resolved = validate_preview_path(&link, &root).expect("canonicalize should succeed");
        assert!(resolved.is_none(), "symlink escaping save_root must be rejected");

        std::fs::remove_dir_all(&base).ok();
    }

    // 回归：缓存键 = 调用方传入的原始路径字符串（非 canonical）。
    // handler 用原始请求路径 get_or_load，失效点（file_index 删除 / exif 注入）
    // 也传原始字符串——两边必须精确匹配，否则失效变 no-op（spec review finding）。
    #[test]
    fn invalidate_raw_path_removes_entry_loaded_via_same_raw_path() {
        let dir = std::env::temp_dir().join("cameraftp_test_cache_raw_key");
        std::fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("test.jpg");
        let mut f = std::fs::File::create(&file_path).unwrap();
        f.write_all(&[0xFF, 0xD8, 0x00, 0x01]).unwrap();

        let cache = ImagePreviewCache::new();
        // 模拟 handler：以原始请求路径为键加载
        cache.get_or_load(&file_path).unwrap();

        // 模拟失效调用点：以同一原始字符串失效
        cache.invalidate(&file_path);

        let inner = cache.inner.read().unwrap();
        assert!(
            !inner.data.contains_key(&cache_key(&file_path)),
            "invalidate with the raw request path must remove the entry"
        );
        assert_eq!(inner.total_bytes, 0);

        drop(inner);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn invalidate_still_works_after_underlying_file_is_deleted() {
        // 键不依赖文件存在性：文件删除后 invalidate（file_index 删除路径的真实
        // 时序——先删文件再失效）仍必须移除条目，防止已删文件的陈旧预览。
        let dir = std::env::temp_dir().join("cameraftp_test_cache_deleted_key");
        std::fs::create_dir_all(&dir).unwrap();
        let file_path = dir.join("gone.jpg");
        std::fs::write(&file_path, b"bytes").unwrap();

        let cache = ImagePreviewCache::new();
        cache.get_or_load(&file_path).unwrap();

        std::fs::remove_file(&file_path).unwrap();
        cache.invalidate(&file_path);

        let inner = cache.inner.read().unwrap();
        assert!(
            !inner.data.contains_key(&cache_key(&file_path)),
            "invalidate must work even after the file is deleted from disk"
        );
        assert_eq!(inner.total_bytes, 0);

        drop(inner);
        std::fs::remove_dir_all(&dir).ok();
    }

    // 回归：缓存键做分隔符归一（cache_key）。索引的两个 raw 路径生产者
    // 拼写不同——scan/watcher 的 Windows PathBuf 反斜杠 vs FTP 正斜杠——
    // 若 get_or_load/invalidate 不用同一归一规则，跨生产者的失效会变
    // no-op，已删/已改文件的陈旧预览继续命中（spec review finding）。
    #[test]
    fn invalidate_cross_producer_separator_spelling_removes_entry() {
        let dir = std::env::temp_dir().join("cameraftp_test_cache_key_norm");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // dir.join 产生平台原生分隔符（Windows 为反斜杠）
        let backslash_path = dir.join("norm.jpg");
        std::fs::write(&backslash_path, b"jpeg-bytes").unwrap();

        let cache = ImagePreviewCache::new();
        // 生产者 A（scan/watcher）：PathBuf 原生拼写装载
        cache.get_or_load(&backslash_path).unwrap();
        assert_eq!(cache.inner.read().unwrap().total_bytes, b"jpeg-bytes".len());

        // 生产者 B（FTP）：正斜杠拼写失效——归一后必须命中同一键
        let forward_path = backslash_path.to_string_lossy().replace('\\', "/");
        cache.invalidate(std::path::Path::new(&forward_path));

        let inner = cache.inner.read().unwrap();
        assert!(
            !inner.data.contains_key(&cache_key(&backslash_path)),
            "forward-slash invalidate must remove the backslash-loaded entry"
        );
        assert!(
            !inner.data.contains_key(&cache_key(std::path::Path::new(&forward_path))),
            "no separator-variant residue may survive"
        );
        assert_eq!(inner.total_bytes, 0, "byte accounting must drop to zero");
        assert_eq!(inner.order.len(), 0, "LRU order must drop to zero");

        drop(inner);
        std::fs::remove_dir_all(&dir).ok();
    }
}
