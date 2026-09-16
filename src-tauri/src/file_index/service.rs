// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;
use futures::StreamExt;
use tokio::sync::RwLock;
#[cfg(target_os = "windows")]
use tokio::sync::Mutex;
use tracing::{info, trace, warn};
#[cfg(target_os = "windows")]
use tracing::error;

use crate::config::AppConfig;
use crate::config_service::ConfigService;
use crate::error::AppError;
use super::types::{FileIndex, FileInfo};
use tauri::Emitter;
#[cfg(target_os = "windows")]
use super::watcher::FileWatcher;

/// 扫描期 get_file_info 并发度。EXIF 解析在 tokio spawn_blocking 池上执行，
/// 池大小天然限流；此值只限制同时在飞的 future 数，避免瞬时占满阻塞池。
const SCAN_CONCURRENCY: usize = 6;

pub struct FileIndexService {
    index: RwLock<FileIndex>,
    save_path: RwLock<PathBuf>,
    #[cfg(target_os = "windows")]
    watcher: Mutex<Option<FileWatcher>>,
    app_handle: Arc<RwLock<Option<tauri::AppHandle>>>,
}

impl FileIndexService {
    pub fn new(config_service: Arc<ConfigService>) -> Self {
        let config = config_service.get().unwrap_or_else(|e| {
            warn!(error = %e, "Failed to read config from ConfigService, using defaults");
            Arc::new(AppConfig::default())
        });
        let save_path = config.save_path.clone();
        Self {
            index: RwLock::new(FileIndex::new()),
            save_path: RwLock::new(save_path.clone()),
            #[cfg(target_os = "windows")]
            watcher: Mutex::new(Some(FileWatcher::new(save_path))),
            app_handle: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn set_app_handle(&self, app_handle: tauri::AppHandle) {
        *self.app_handle.write().await = Some(app_handle);
    }

    /// 发射文件索引变化事件到前端
    async fn emit_file_index_changed(&self) {
        let app_handle_opt = {
            let guard = self.app_handle.read().await;
            guard.clone()
        };

        if let Some(ref app_handle) = app_handle_opt {
            let count;
            let latest_filename;
            {
                let index = self.index.read().await;
                count = index.files().len();
                latest_filename = index.files().first().map(|f| f.filename.clone());
            }
            trace!("File index changed event emitted: count={}, latest={:?}", count, latest_filename);
            if let Err(e) = app_handle.emit(
                "file-index-changed",
                serde_json::json!({
                    "count": count,
                    "latestFilename": latest_filename
                }),
            ) {
                warn!(event = "file-index-changed", error = %e, "Failed to emit frontend event");
            }
        }
    }

    /// 启动文件系统监听（仅 Windows）
    /// 注意：需要传入 Arc<Self> 以在 watcher 任务中保持服务存活
    #[cfg_attr(target_os = "android", allow(unused_variables))]
    pub async fn start_watcher(self_arc: Arc<Self>) -> Result<bool, AppError> {
        #[cfg(target_os = "windows")]
        {
            // 先检查/创建 watcher
            let save_path = self_arc.save_path.read().await.clone();
            {
                let mut watcher_guard = self_arc.watcher.lock().await;
                if watcher_guard.is_none() {
                    *watcher_guard = Some(FileWatcher::new(save_path));
                }
            } // 释放 watcher_guard

            // 取出 watcher 的所有权，调用 start 后放回 Mutex
            let watcher_option = {
                let mut watcher_guard = self_arc.watcher.lock().await;
                watcher_guard.take() // 将 watcher 从 Mutex 中取出
            };

            if let Some(mut watcher) = watcher_option {
                // 克隆 Arc 用于 watcher 任务
                let self_arc_clone = Arc::clone(&self_arc);

                let result = watcher.start(self_arc_clone).await;

                // 将 watcher 重新放回 Mutex（无论 start 成功与否）
                {
                    let mut watcher_guard = self_arc.watcher.lock().await;
                    *watcher_guard = Some(watcher);
                }

                match result {
                    Ok(true) => {
                        info!("File watcher started successfully");
                        Ok(true)
                    }
                    Ok(false) => {
                        info!("File watcher not started (may be unsupported platform)");
                        Ok(false)
                    }
                    Err(e) => {
                        error!("Failed to start file watcher: {}", e);
                        Err(AppError::Other(format!("Failed to start watcher: {}", e)))
                    }
                }
            } else {
                Ok(false)
            }
        }

        #[cfg(target_os = "android")]
        {
            // Android 不使用文件系统监听
            info!("File watcher is disabled on Android");
            Ok(false)
        }
    }

    /// 停止文件系统监听
    #[cfg(target_os = "windows")]
    pub async fn stop_watcher(&self) {
        let mut watcher_guard = self.watcher.lock().await;
        // 置 None 让 start_watcher 能用新路径重建 FileWatcher
        // FileWatcher 的 Drop 会停止内部 RecommendedWatcher
        if watcher_guard.take().is_some() {
            info!("File watcher stopped");
        }
    }

    /// 停止文件系统监听（Android 平台 - 无操作）
    #[cfg(target_os = "android")]
    pub async fn stop_watcher(&self) {
        // Android 不使用文件系统监听
    }

    /// 扫描目录建立索引
    pub async fn scan_directory(&self) -> Result<(), AppError> {
        let save_path = self.save_path.read().await.clone();
        info!("Starting directory scan: {:?}", save_path);

        // 提交前快照：识别"扫描期间通过 FTP Put 通道新进入索引"的文件，
        // 提交时保留它们（见 merge_scan_result）
        let pre_scan_paths: HashSet<PathBuf> = {
            let index = self.index.read().await;
            index.path_set.clone()
        };

        let paths = self.collect_image_paths(&save_path).await?;

        // 并发获取文件信息（EXIF 解析经 spawn_blocking，见 read_exif_time）。
        // 错误携带源路径，日志可定位到具体文件。
        let infos = {
            futures::stream::iter(paths)
                .map(|path| async move {
                    let metadata = match tokio::fs::metadata(&path).await {
                        Ok(m) => m,
                        Err(e) => {
                            return Err((path, AppError::Other(format!("Failed to get metadata: {}", e))))
                        }
                    };
                    self.get_file_info(&path, &metadata).await.map_err(|e| (path, e))
                })
                .buffer_unordered(SCAN_CONCURRENCY)
                .collect::<Vec<Result<FileInfo, (PathBuf, AppError)>>>()
                .await
        };

        let files: Vec<FileInfo> = infos
            .into_iter()
            .filter_map(|r| match r {
                Ok(file_info) => Some(file_info),
                Err((path, e)) => {
                    warn!("Failed to get file info during scan {:?}: {}", path, e);
                    None
                }
            })
            .collect();

        let mut index = self.index.write().await;
        // merge_scan_result 内部会整体排序，此处无需预排序；
        // existing 只被只读借用，克隆外层 Arc 即可，避免整份 Vec 深拷贝
        let existing = Arc::clone(index.files());
        let merged = Self::merge_scan_result(files, &existing, &pre_scan_paths);
        index.current_index = merged.first().map(|_| 0);
        let count = merged.len();
        index.set_files(merged);

        info!("Directory scan complete: {} files found", count);

        drop(index);
        self.emit_file_index_changed().await;

        Ok(())
    }

    /// 迭代遍历目录（工作栈），收集受支持的图片路径
    async fn collect_image_paths(&self, root: &Path) -> Result<Vec<PathBuf>, AppError> {
        let mut dirs_to_process = vec![root.to_path_buf()];
        let mut paths = Vec::new();

        while let Some(dir) = dirs_to_process.pop() {
            let mut entries = tokio::fs::read_dir(&dir).await
                .map_err(|e| AppError::Other(format!("Failed to read dir: {}", e)))?;

            while let Some(entry) = entries.next_entry().await
                .map_err(|e| AppError::Other(format!("Failed to read entry: {}", e)))?
            {
                let path = entry.path();
                let metadata = match entry.metadata().await {
                    Ok(m) => m,
                    Err(_) => continue,
                };

                if metadata.is_dir() {
                    dirs_to_process.push(path);
                } else if metadata.is_file()
                    && crate::image_utils::is_supported_image(&path) {
                        paths.push(path);
                    }
            }
        }

        Ok(paths)
    }

    /// 合并扫描结果与索引中"扫描期间新增"的条目。
    ///
    /// FTP Put 监听（ftp/listeners.rs）独立 spawn 调 add_file，可在扫描的
    /// readdir 与 set_files 提交之间插入。保留规则：
    /// - existing 中路径不在 scanned 结果、也不在 pre_scan_paths 快照中
    ///   → 扫描开始后才进入索引的新文件，保留；
    /// - 在 pre_scan_paths 中但不在 scanned 中 → 扫描期间已删除，丢弃
    ///   （与旧 set_files 整体覆盖行为一致）；
    /// - 同一路径以 scanned 的新元数据为准。
    fn merge_scan_result(
        scanned: Vec<FileInfo>,
        existing: &[FileInfo],
        pre_scan_paths: &HashSet<PathBuf>,
    ) -> Vec<FileInfo> {
        // 先在借用期内完成过滤（scanned_paths 持有对 scanned 的借用）
        let added_during_scan: Vec<FileInfo> = {
            let scanned_paths: HashSet<&PathBuf> = scanned.iter().map(|f| &f.path).collect();
            existing
                .iter()
                .filter(|f| !scanned_paths.contains(&f.path) && !pre_scan_paths.contains(&f.path))
                .cloned()
                .collect()
        };

        let mut merged = scanned;
        merged.extend(added_during_scan);

        merged.sort_by(|a, b| {
            b.sort_time.cmp(&a.sort_time)
                .then_with(|| b.modified_time.cmp(&a.modified_time))
        });
        merged
    }



    /// 获取文件信息（包括EXIF时间）
    async fn get_file_info(&self, path: &Path, metadata: &std::fs::Metadata) -> Result<FileInfo, AppError> {
        let filename = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        
        let modified_time = metadata.modified()
            .unwrap_or(SystemTime::UNIX_EPOCH);
        
        // 尝试读取EXIF时间
        let exif_time = self.read_exif_time(path).await;
        
        // sort_time 优先使用 exif_time
        let sort_time = exif_time.unwrap_or(modified_time);
        let sort_time_ms = sort_time
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        Ok(FileInfo {
            path: path.to_path_buf(),
            filename,
            exif_time,
            modified_time,
            sort_time: sort_time_ms,
        })
    }

    /// 读取图片EXIF中的拍摄时间
    async fn read_exif_time(&self, path: &Path) -> Option<SystemTime> {
        let path = path.to_path_buf();
        tokio::task::spawn_blocking(move || -> Option<SystemTime> {
            crate::image_utils::parse_exif(&path).ok()??
                .datetime_original
                .map(|ndt| ndt.and_utc().into())
        }).await.ok()?
    }

    /// 添加新文件（FTP 上传/watcher 事件调用）
    ///
    /// watcher（Created，经稳定性探测放行）与 FTP Put 监听器（文件完全
    /// 写入后触发）会对同一上传文件各调一次，因此：
    /// 1. EXIF 解析（spawn_blocking 全文件扫描）之前先做廉价查重；
    /// 2. 已存在但缺 EXIF 的陈旧条目（写入中途索引的遗留）重新解析回填。
    pub async fn add_file(&self, path: PathBuf) -> Result<(), AppError> {
        if !crate::image_utils::is_supported_image(&path) {
            return Ok(()); // 跳过非图片文件
        }

        // 廉价查重前置：已索引且不缺 EXIF → 直接返回，避免重复解析
        {
            let index = self.index.read().await;
            if index.contains_path(&path) {
                let needs_backfill = index
                    .files()
                    .iter()
                    .any(|f| f.path == path && f.exif_time.is_none());
                if !needs_backfill {
                    trace!("File already indexed, skipping: {:?}", path);
                    return Ok(());
                }
            }
        }

        let metadata = tokio::fs::metadata(&path).await
            .map_err(|e| AppError::Other(format!("Failed to get metadata: {}", e)))?;

        let file_info = self.get_file_info(&path, &metadata).await?;

        // 原子检查-插入（写锁内防 TOCTOU 竞态）
        let mut index = self.index.write().await;

        // 单次遍历定位：同时携带位置、既有 EXIF 状态与"是否为当前查看项"，
        // 避免对 files() 的重复索引访问
        let mut was_current = false;
        if let Some((pos, existing)) = index.files().iter().enumerate().find(|(_, f)| f.path == path) {
            let existing_has_exif = existing.exif_time.is_some();
            was_current = index.current_index == Some(pos);
            if existing_has_exif || file_info.exif_time.is_none() {
                // 已有条目不劣于新解析结果：跳过（并发重复，或新解析失败）
                trace!("File already indexed, skipping: {:?}", path);
                return Ok(());
            }
            // 回填：移除缺 EXIF 的陈旧条目，让新 file_info 按排序位置重新插入
            let files: &mut Vec<FileInfo> = Arc::make_mut(&mut index.files);
            files.remove(pos);
            let new_len = files.len();
            Self::adjust_current_index_after_removal(&mut index.current_index, pos, new_len);
            index.path_set.remove(&path);
            info!("Backfilled EXIF for stale index entry: {:?}", path);
        }

        // Insert into sorted position using copy-on-write (Arc::make_mut)
        {
            let files: &mut Vec<FileInfo> = Arc::make_mut(&mut index.files);
            // sort_time 降序，相同则 modified_time 降序（新文件优先）
            let insert_pos = files.iter()
                .position(|f| {
                    f.sort_time < file_info.sort_time ||
                    (f.sort_time == file_info.sort_time && f.modified_time < file_info.modified_time)
                })
                .unwrap_or(files.len());

            files.insert(insert_pos, file_info);

            if let Some(current) = index.current_index {
                if insert_pos <= current {
                    index.current_index = Some(current + 1);
                }
            }
            // 回填重插的正是当前查看项：remove+adjust 后的通用位移不再适用，
            // 无论条目前移/后移，直接把 current_index 指回该文件的新位置
            if was_current {
                index.current_index = Some(insert_pos);
            }
        }

        index.path_set.insert(path.clone());
        drop(index);
        info!("Added file to index: {:?}", path);

        // 发射文件索引变化事件
        self.emit_file_index_changed().await;

        Ok(())
    }

    /// 从索引中移除文件
    pub async fn remove_file(&self, path: &Path) -> Result<bool, AppError> {
        let mut index = self.index.write().await;

        if let Some(pos) = index.files().iter().position(|f| f.path == path) {
            let new_len = {
                let files: &mut Vec<FileInfo> = Arc::make_mut(&mut index.files);
                files.remove(pos);
                files.len()
            };
            index.path_set.remove(path);

            Self::adjust_current_index_after_removal(&mut index.current_index, pos, new_len);

            drop(index);
            info!("Removed file from index: {:?}", path);

            // 失效预览缓存中该文件的条目，避免已删除文件的预览继续命中旧数据
            self.invalidate_preview_cache(path).await;

            // 发射文件索引变化事件
            self.emit_file_index_changed().await;

            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// 使指定路径的预览缓存条目失效（若缓存可用）
    /// 预览缓存模块仅在 Windows 启用（见 lib.rs 的 cfg 声明），
    /// 其他平台的删除路径无需失效操作。
    async fn invalidate_preview_cache(&self, path: &Path) {
        #[cfg(target_os = "windows")]
        {
            let app_handle = {
                let guard = self.app_handle.read().await;
                guard.clone()
            };

            if let Some(app_handle) = app_handle {
                use tauri::Manager;
                if let Some(cache) = app_handle.try_state::<Arc<crate::image_preview::ImagePreviewCache>>() {
                    cache.invalidate(path);
                }
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = path;
        }
    }

    /// 获取文件列表
    pub async fn get_files(&self) -> Arc<Vec<FileInfo>> {
        let index = self.index.read().await;
        index.files().clone()
    }

    /// 获取当前索引
    pub async fn get_current_index(&self) -> Option<usize> {
        let index = self.index.read().await;
        index.current_index
    }

    /// 导航到指定索引
    /// 如果目标文件不存在，会尝试清理并返回错误
    pub async fn navigate_to(&self, new_index: usize) -> Result<FileInfo, AppError> {
        let file_info = self.get_file_at_index(new_index).await?;

        if !self.verify_file_exists(&file_info.path).await {
            self.remove_missing_file(&file_info.path).await;
            return Err(AppError::Other(format!(
                "File not found: {}",
                file_info.path.display()
            )));
        }

        self.set_current_index(new_index).await;
        Ok(file_info)
    }

    /// 获取指定索引处的文件信息
    async fn get_file_at_index(&self, index: usize) -> Result<FileInfo, AppError> {
        let idx = self.index.read().await;
        if index >= idx.files().len() {
            return Err(AppError::Other("Index out of bounds".to_string()));
        }
        Ok(idx.files()[index].clone())
    }

    /// 验证文件是否存在
    async fn verify_file_exists(&self, path: &Path) -> bool {
        tokio::fs::try_exists(path).await.unwrap_or(false)
    }

    /// 从索引中移除不存在的文件，并调整当前索引
    async fn remove_missing_file(&self, path: &Path) {
        let mut index = self.index.write().await;
        let Some(pos) = index.files().iter().position(|f| f.path == path) else {
            return;
        };

        let new_len = {
            let files: &mut Vec<FileInfo> = Arc::make_mut(&mut index.files);
            files.remove(pos);
            files.len()
        };
        index.path_set.remove(path);
        Self::adjust_current_index_after_removal(&mut index.current_index, pos, new_len);
        info!("Removed missing file from index: {:?}", path);
    }

    /// 移除文件后调整当前索引
    fn adjust_current_index_after_removal(
        current_index: &mut Option<usize>,
        removed_pos: usize,
        new_len: usize,
    ) {
        let Some(current) = current_index else { return };

        if new_len == 0 {
            *current_index = None;
        } else if removed_pos < *current {
            *current_index = Some(*current - 1);
        } else if removed_pos == *current && *current >= new_len {
            *current_index = Some(new_len - 1);
        }
    }

    /// 设置当前索引
    async fn set_current_index(&self, new_index: usize) {
        let mut index = self.index.write().await;
        index.current_index = Some(new_index);
    }

    /// 获取最新文件（排序第一个）
    pub async fn get_latest_file(&self) -> Option<FileInfo> {
        {
            let index = self.index.read().await;
            if let Some(file) = index.files().first() {
                return Some(file.clone());
            }
        }

        if let Err(e) = self.scan_directory().await {
            warn!(error = %e, "Failed to scan directory while getting latest file");
            return None;
        }

        let index = self.index.read().await;
        index.files().first().cloned()
    }

    /// 根据文件路径查找索引
    pub async fn find_file_index(&self, path: &Path) -> Option<usize> {
        let index = self.index.read().await;
        index.files().iter().position(|f| f.path == path)
    }

    /// 获取文件数量
    #[cfg(test)]
    pub async fn get_file_count(&self) -> usize {
        let index = self.index.read().await;
        index.files().len()
    }

    #[cfg(test)]
    pub async fn set_test_files(&self, files: Vec<FileInfo>) {
        let mut index = self.index.write().await;
        index.set_files(files);
        index.current_index = if !index.files().is_empty() { Some(0) } else { None };
    }

    pub async fn update_save_path(self: Arc<Self>, new_path: PathBuf) -> Result<(), AppError> {
        let current_path = self.save_path.read().await.clone();
        if current_path == new_path {
            return Ok(());
        }

        info!(
            "Updating save_path from {:?} to {:?}",
            current_path, new_path
        );

        self.stop_watcher().await;
        *self.save_path.write().await = new_path;
        self.scan_directory().await?;
        Self::start_watcher(Arc::clone(&self)).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::time::SystemTime;

    use tempfile::tempdir;

    use crate::config_service::ConfigService;
    use crate::file_index::types::FileInfo;

    use super::FileIndexService;

    fn make_file_info(path: &str, sort_time_ms: u64) -> FileInfo {
        FileInfo {
            path: PathBuf::from(path),
            filename: path.split('/').last().unwrap_or(path).to_string(),
            exif_time: None,
            modified_time: SystemTime::UNIX_EPOCH,
            sort_time: sort_time_ms,
        }
    }

    #[tokio::test]
    async fn get_latest_file_windows_startup_returns_saved_image_without_prior_scan() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let config_path = temp_dir.path().join("config.json");
        let save_path = temp_dir.path().join("images");

        std::fs::create_dir_all(&save_path).expect("failed to create save path");
        std::fs::write(save_path.join("latest.jpg"), b"test-jpeg-content")
            .expect("failed to write image file");

        let config_service = ConfigService::new_with_path(config_path);
        config_service
            .mutate_and_persist(|config| {
                config.save_path = PathBuf::from(&save_path);
            })
            .expect("failed to update save path in config");

        let file_index_service = FileIndexService::new(Arc::new(config_service));

        assert_eq!(file_index_service.get_file_count().await, 0);

        let latest = file_index_service.get_latest_file().await;

        assert!(
            latest.is_some(),
            "expected latest file on startup from configured save path even before explicit scan"
        );
        assert_eq!(
            latest
                .as_ref()
                .expect("latest file should exist")
                .filename,
            "latest.jpg"
        );
        assert_eq!(file_index_service.get_file_count().await, 1);
    }

    #[tokio::test]
    async fn remove_file_adjusts_current_index_when_removing_before_current() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let config_path = temp_dir.path().join("config.json");
        let config_service = ConfigService::new_with_path(config_path);
        let service = FileIndexService::new(Arc::new(config_service));

        service.set_test_files(vec![
            make_file_info("/images/a.jpg", 3000),
            make_file_info("/images/b.jpg", 2000),
            make_file_info("/images/c.jpg", 1000),
        ]).await;

        assert_eq!(service.get_current_index().await, Some(0));

        // Remove file after current — index should stay 0
        let removed = service.remove_file(Path::new("/images/c.jpg")).await;
        assert!(removed.expect("remove should succeed"));
        assert_eq!(service.get_file_count().await, 2);
        assert_eq!(service.get_current_index().await, Some(0));
    }

    #[tokio::test]
    async fn remove_file_at_current_stays_at_zero() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let config_path = temp_dir.path().join("config.json");
        let config_service = ConfigService::new_with_path(config_path);
        let service = FileIndexService::new(Arc::new(config_service));

        service.set_test_files(vec![
            make_file_info("/images/a.jpg", 3000),
            make_file_info("/images/b.jpg", 2000),
        ]).await;

        // Remove current (index 0 = a.jpg) — should stay at 0, now pointing to b.jpg
        let removed = service.remove_file(Path::new("/images/a.jpg")).await;
        assert!(removed.expect("remove should succeed"));
        assert_eq!(service.get_current_index().await, Some(0));
        let files = service.get_files().await;
        assert_eq!(files[0].filename, "b.jpg");
    }

    #[tokio::test]
    async fn remove_nonexistent_file_returns_false() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let config_path = temp_dir.path().join("config.json");
        let config_service = ConfigService::new_with_path(config_path);
        let service = FileIndexService::new(Arc::new(config_service));

        service.set_test_files(vec![
            make_file_info("/images/a.jpg", 3000),
        ]).await;

        let removed = service.remove_file(Path::new("/images/nonexistent.jpg")).await;
        assert!(!removed.expect("remove should return false"));
        assert_eq!(service.get_file_count().await, 1);
    }

    #[tokio::test]
    async fn get_latest_file_returns_newest_by_sort_time() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let config_path = temp_dir.path().join("config.json");
        let config_service = ConfigService::new_with_path(config_path);
        let service = FileIndexService::new(Arc::new(config_service));

        service.set_test_files(vec![
            make_file_info("/images/newest.jpg", 3000),
            make_file_info("/images/oldest.jpg", 1000),
        ]).await;

        let latest = service.get_latest_file().await;
        assert!(latest.is_some());
        assert_eq!(latest.unwrap().filename, "newest.jpg");
    }

    #[tokio::test]
    async fn add_file_skips_duplicate_path() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let config_path = temp_dir.path().join("config.json");
        let save_path = temp_dir.path().join("images");
        std::fs::create_dir_all(&save_path).expect("create dir");

        let config_service = ConfigService::new_with_path(config_path);
        config_service
            .mutate_and_persist(|config| {
                config.save_path = save_path.clone();
            })
            .expect("persist config");

        let service = FileIndexService::new(Arc::new(config_service));

        // Create a real file
        let file_path = save_path.join("test.jpg");
        std::fs::write(&file_path, b"test-jpeg-content").expect("write file");

        // Add it
        service.add_file(file_path.clone()).await.expect("first add");
        assert_eq!(service.get_file_count().await, 1);

        // Add again — should be skipped as duplicate
        service.add_file(file_path.clone()).await.expect("second add");
        assert_eq!(service.get_file_count().await, 1);
    }

    #[tokio::test]
    async fn add_file_backfills_exif_for_stale_entry() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let config_path = temp_dir.path().join("config.json");
        let save_path = temp_dir.path().join("images");
        std::fs::create_dir_all(&save_path).expect("create dir");

        let config_service = ConfigService::new_with_path(config_path);
        config_service
            .mutate_and_persist(|config| {
                config.save_path = save_path.clone();
            })
            .expect("persist config");

        let service = FileIndexService::new(Arc::new(config_service));

        let file_path = save_path.join("backfill.jpg");

        // 1) 无 EXIF 的普通 JPEG 先入索引（模拟"写入中途索引"的陈旧条目）
        let plain = image::RgbImage::from_pixel(2, 2, image::Rgb([64u8, 64, 64]));
        let mut buf: Vec<u8> = Vec::new();
        image::DynamicImage::ImageRgb8(plain)
            .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Jpeg)
            .expect("encode plain jpeg");
        std::fs::write(&file_path, &buf).expect("write plain jpeg");
        // mtime 拨到一年前，确保陈旧条目与回填后的 sort_time 差异明显
        let old_system = std::time::SystemTime::now()
            - std::time::Duration::from_secs(365 * 24 * 3600);
        filetime::set_file_mtime(
            &file_path,
            filetime::FileTime::from_system_time(old_system),
        )
        .expect("set old mtime");

        service.add_file(file_path.clone()).await.expect("first add");
        let files = service.get_files().await;
        assert_eq!(files.len(), 1);
        assert!(files[0].exif_time.is_none(), "plain jpeg entry has no exif");

        // 2) 文件补上 EXIF（等价于完整写入后 Put 事件再次触发 add_file）
        std::fs::write(
            &file_path,
            crate::image_utils::build_exif_jpeg("2024:06:01 12:00:00", 1),
        )
        .expect("overwrite with exif jpeg");

        service.add_file(file_path.clone()).await.expect("second add");

        let files = service.get_files().await;
        assert_eq!(files.len(), 1, "backfill must replace, not duplicate");
        assert!(
            files[0].exif_time.is_some(),
            "stale entry must be backfilled with EXIF time"
        );
    }

    #[tokio::test]
    async fn add_file_backfill_keeps_current_index_when_exif_moves_newer() {
        // 当前查看项回填后 EXIF 变新 → 条目前移到列表头（位置 0），
        // current_index 必须仍指向该文件，而不是指向顶替其旧位置的邻居
        let temp_dir = tempdir().expect("failed to create temp dir");
        let config_service = ConfigService::new_with_path(temp_dir.path().join("config.json"));
        let service = FileIndexService::new(Arc::new(config_service));

        // [a(3000), b(2000), c(1000)] 按 mtime 排序
        service.add_file(create_file_with_mtime(temp_dir.path(), "a.jpg", 3000)).await.unwrap();
        service.add_file(create_file_with_mtime(temp_dir.path(), "b.jpg", 2000)).await.unwrap();
        service.add_file(create_file_with_mtime(temp_dir.path(), "c.jpg", 1000)).await.unwrap();

        // 目标文件：无 EXIF、mtime 1500 → 落在 b 与 c 之间（索引位置 2），设为当前查看项
        let target = create_file_with_mtime(temp_dir.path(), "target.jpg", 1500);
        service.add_file(target.clone()).await.unwrap();
        let current = service.find_file_index(&target).await.expect("target must be indexed");
        assert_eq!(current, 2);
        service.navigate_to(current).await.expect("navigate to target");
        assert_eq!(service.get_current_index().await, Some(2));

        // 补上"远新于一切 mtime"的 EXIF（2030）→ 回填后 sort_time 最大，前移到位置 0
        std::fs::write(&target, crate::image_utils::build_exif_jpeg("2030:06:15 12:00:00", 1))
            .expect("overwrite with exif jpeg");
        service.add_file(target.clone()).await.unwrap();

        let files = service.get_files().await;
        assert_eq!(files.len(), 4, "backfill must replace, not duplicate");
        assert_eq!(files[0].path, target, "newer EXIF must move the entry to the front");
        let current_index = service.get_current_index().await.expect("current must stay set");
        assert_eq!(current_index, 0);
        assert_eq!(files[current_index].path, target, "current index must follow the backfilled file");
    }

    #[tokio::test]
    async fn add_file_backfill_keeps_current_index_when_exif_moves_older() {
        // 当前查看项回填后 EXIF 变旧 → 条目后移到列表尾（位置 3），
        // current_index 必须仍指向该文件
        let temp_dir = tempdir().expect("failed to create temp dir");
        let config_service = ConfigService::new_with_path(temp_dir.path().join("config.json"));
        let service = FileIndexService::new(Arc::new(config_service));

        service.add_file(create_file_with_mtime(temp_dir.path(), "a.jpg", 3000)).await.unwrap();
        service.add_file(create_file_with_mtime(temp_dir.path(), "b.jpg", 2000)).await.unwrap();
        service.add_file(create_file_with_mtime(temp_dir.path(), "c.jpg", 1000)).await.unwrap();

        // 无 EXIF、mtime 1500 → 落在 b 与 c 之间（索引位置 2），设为当前查看项
        let target = create_file_with_mtime(temp_dir.path(), "target.jpg", 1500);
        service.add_file(target.clone()).await.unwrap();
        let current = service.find_file_index(&target).await.expect("target must be indexed");
        assert_eq!(current, 2);
        service.navigate_to(current).await.expect("navigate to target");

        // 补上"远旧于一切 mtime"的 EXIF（epoch+30s）→ 回填后 sort_time 最小，后移到末尾
        std::fs::write(&target, crate::image_utils::build_exif_jpeg("1970:01:01 00:00:30", 1))
            .expect("overwrite with exif jpeg");
        service.add_file(target.clone()).await.unwrap();

        let files = service.get_files().await;
        assert_eq!(files.len(), 4, "backfill must replace, not duplicate");
        assert_eq!(files[3].path, target, "older EXIF must move the entry to the back");
        let current_index = service.get_current_index().await.expect("current must stay set");
        assert_eq!(current_index, 3);
        assert_eq!(files[current_index].path, target, "current index must follow the backfilled file");
    }

    #[tokio::test]
    async fn remove_all_files_clears_current_index() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let config_path = temp_dir.path().join("config.json");
        let config_service = ConfigService::new_with_path(config_path);
        let service = FileIndexService::new(Arc::new(config_service));

        service.set_test_files(vec![
            make_file_info("/images/a.jpg", 3000),
        ]).await;

        assert_eq!(service.get_current_index().await, Some(0));

        // Remove the only file — index should become None
        let removed = service.remove_file(Path::new("/images/a.jpg")).await;
        assert!(removed.expect("remove should succeed"));
        assert_eq!(service.get_file_count().await, 0);
        assert_eq!(service.get_current_index().await, None);
    }

    #[tokio::test]
    async fn empty_index_returns_none_for_latest() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let config_path = temp_dir.path().join("config.json");
        let save_path = temp_dir.path().join("empty_images");
        std::fs::create_dir_all(&save_path).expect("create dir");

        let config_service = ConfigService::new_with_path(config_path);
        config_service
            .mutate_and_persist(|config| {
                config.save_path = save_path.clone();
            })
            .expect("persist config");

        let service = FileIndexService::new(Arc::new(config_service));

        let latest = service.get_latest_file().await;
        assert!(latest.is_none(), "empty directory should return None");
    }

    // ---- Sorted-insert tests: current_index shifts and sort-time precedence ----

    fn create_file_with_mtime(dir: &Path, name: &str, mtime_secs: i64) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, b"test-jpeg-content").expect("write file");
        filetime::set_file_mtime(&path, filetime::FileTime::from_unix_time(mtime_secs, 0))
            .expect("set mtime");
        path
    }

    #[tokio::test]
    async fn add_file_newer_than_current_shifts_current_index() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let config_service = ConfigService::new_with_path(temp_dir.path().join("config.json"));
        let service = FileIndexService::new(Arc::new(config_service));

        // Seed: [b(2000), a(1000)] sorted by sort_time descending
        service.add_file(create_file_with_mtime(temp_dir.path(), "b.jpg", 2000)).await.unwrap();
        service.add_file(create_file_with_mtime(temp_dir.path(), "a.jpg", 1000)).await.unwrap();
        service.navigate_to(0).await.expect("navigate to first file");
        assert_eq!(service.get_current_index().await, Some(0));

        // Insert a NEWER file — it lands before the current index → shift +1
        service.add_file(create_file_with_mtime(temp_dir.path(), "n.jpg", 3000)).await.unwrap();

        assert_eq!(service.get_current_index().await, Some(1), "current index must shift past the inserted file");
        let files = service.get_files().await;
        assert_eq!(files.len(), 3);
        assert_eq!(files[0].filename, "n.jpg");
        assert_eq!(files[1].filename, "b.jpg", "current index must still point at b.jpg");
        assert_eq!(files[2].filename, "a.jpg");
    }

    #[tokio::test]
    async fn add_file_older_than_current_keeps_current_index() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let config_service = ConfigService::new_with_path(temp_dir.path().join("config.json"));
        let service = FileIndexService::new(Arc::new(config_service));

        service.add_file(create_file_with_mtime(temp_dir.path(), "b.jpg", 2000)).await.unwrap();
        service.add_file(create_file_with_mtime(temp_dir.path(), "a.jpg", 1000)).await.unwrap();
        service.navigate_to(0).await.expect("navigate to first file");
        assert_eq!(service.get_current_index().await, Some(0));

        // Insert an OLDER file — it lands after the current index → no shift
        service.add_file(create_file_with_mtime(temp_dir.path(), "c.jpg", 500)).await.unwrap();

        assert_eq!(service.get_current_index().await, Some(0), "insert after current must not shift");
        let files = service.get_files().await;
        assert_eq!(files.len(), 3);
        assert_eq!(files[0].filename, "b.jpg", "current index must still point at b.jpg");
        assert_eq!(files[1].filename, "a.jpg");
        assert_eq!(files[2].filename, "c.jpg");
    }

    #[tokio::test]
    async fn add_file_sorts_by_exif_time_over_newer_mtime() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let config_service = ConfigService::new_with_path(temp_dir.path().join("config.json"));
        let service = FileIndexService::new(Arc::new(config_service));

        // Old mtime (1000s) but EXIF DateTimeOriginal far in the future (2030)
        let exif_path = temp_dir.path().join("exif_new.jpg");
        std::fs::write(
            &exif_path,
            crate::image_utils::build_exif_jpeg("2030:06:15 12:00:00", 1),
        )
        .expect("write exif jpeg");
        filetime::set_file_mtime(&exif_path, filetime::FileTime::from_unix_time(1000, 0))
            .expect("set old mtime");

        // No EXIF, newest mtime (5000s) — still far older than the 2030 EXIF time
        let mtime_path = create_file_with_mtime(temp_dir.path(), "mtime_new.jpg", 5000);

        // Insertion order must not matter: mtime-only file added first
        service.add_file(mtime_path).await.unwrap();
        service.add_file(exif_path).await.unwrap();

        let files = service.get_files().await;
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].filename, "exif_new.jpg", "EXIF time must win over newer mtime");
        assert_eq!(files[1].filename, "mtime_new.jpg");
        assert!(files[0].exif_time.is_some(), "EXIF time must be recorded for the EXIF file");
        assert!(files[1].exif_time.is_none(), "no EXIF file must fall back to mtime");
        assert!(files[0].sort_time > files[1].sort_time);
    }

    // ---- 并发扫描与合并提交 ----

    #[test]
    fn merge_scan_result_preserves_files_added_during_scan() {
        let scanned = vec![make_file_info("/images/a.jpg", 3000)];
        let existing = vec![
            make_file_info("/images/a.jpg", 3000),
            // FTP Put 通道在 readdir 之后、提交之前插入
            make_file_info("/images/new_from_ftp.jpg", 5000),
        ];
        let pre_scan_paths: HashSet<PathBuf> =
            [PathBuf::from("/images/a.jpg")].into_iter().collect();

        let merged = FileIndexService::merge_scan_result(scanned, &existing, &pre_scan_paths);

        assert_eq!(merged.len(), 2);
        // 排序：sort_time 降序 → new_from_ftp(5000) 在前
        assert_eq!(merged[0].filename, "new_from_ftp.jpg");
    }

    #[test]
    fn merge_scan_result_drops_files_deleted_during_scan() {
        let scanned = vec![make_file_info("/images/a.jpg", 3000)];
        let existing = vec![
            make_file_info("/images/a.jpg", 3000),
            make_file_info("/images/deleted.jpg", 1000),
        ];
        let pre_scan_paths: HashSet<PathBuf> =
            existing.iter().map(|f| f.path.clone()).collect();

        let merged = FileIndexService::merge_scan_result(scanned, &existing, &pre_scan_paths);

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].filename, "a.jpg");
    }

    #[test]
    fn merge_scan_result_prefers_scanned_metadata_for_same_path() {
        let scanned = vec![make_file_info("/images/a.jpg", 9000)];
        let existing = vec![make_file_info("/images/a.jpg", 3000)];
        let pre_scan_paths: HashSet<PathBuf> =
            [PathBuf::from("/images/a.jpg")].into_iter().collect();

        let merged = FileIndexService::merge_scan_result(scanned, &existing, &pre_scan_paths);

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].sort_time, 9000);
    }

    #[tokio::test]
    async fn scan_directory_concurrent_finds_all_images_in_nested_dirs() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let save_path = temp_dir.path().join("images");
        std::fs::create_dir_all(save_path.join("sub_a/sub_b")).expect("create nested dirs");

        let mut expected = std::collections::HashSet::new();
        for i in 0..12 {
            let name = format!("img_{:02}.jpg", i);
            std::fs::write(save_path.join("sub_a").join(&name), b"jpeg").expect("write file");
            expected.insert(name);
        }
        for i in 12..20 {
            let name = format!("img_{:02}.jpg", i);
            std::fs::write(save_path.join("sub_a/sub_b").join(&name), b"jpeg").expect("write file");
            expected.insert(name);
        }
        std::fs::write(save_path.join("notes.txt"), b"not an image").expect("write file");

        let config_service = ConfigService::new_with_path(temp_dir.path().join("config.json"));
        config_service
            .mutate_and_persist(|config| {
                config.save_path = save_path.clone();
            })
            .expect("persist config");

        let service = FileIndexService::new(Arc::new(config_service));
        service.scan_directory().await.expect("scan");

        let files = service.get_files().await;
        assert_eq!(files.len(), 20, "all supported images must be indexed");
        let names: std::collections::HashSet<String> =
            files.iter().map(|f| f.filename.clone()).collect();
        assert_eq!(names, expected);
    }

    // ---- 语义钉住（spec review findings）----

    #[tokio::test]
    async fn scan_directory_commit_resets_current_index_to_latest() {
        // 语义钉住（当前实际行为）：扫描提交把 current_index 重置到最新一张
        //（位置 0，实现为 merged.first().map(|_| 0)），不保留扫描前的查看
        // 位置；空目录提交后为 None。
        let temp_dir = tempdir().expect("failed to create temp dir");
        let save_path = temp_dir.path().join("images");
        std::fs::create_dir_all(&save_path).expect("create dir");

        let config_service = ConfigService::new_with_path(temp_dir.path().join("config.json"));
        config_service
            .mutate_and_persist(|config| {
                config.save_path = save_path.clone();
            })
            .expect("persist config");
        let service = FileIndexService::new(Arc::new(config_service));

        let newest = create_file_with_mtime(&save_path, "newest.jpg", 3000);
        let oldest = create_file_with_mtime(&save_path, "oldest.jpg", 1000);
        service.add_file(newest.clone()).await.unwrap();
        service.add_file(oldest).await.unwrap();
        // 先查看较旧的一张（位置 1），扫描提交后必须回到最新（位置 0）
        service.navigate_to(1).await.expect("navigate to the older entry");
        assert_eq!(service.get_current_index().await, Some(1));

        service.scan_directory().await.expect("rescan");

        assert_eq!(
            service.get_current_index().await,
            Some(0),
            "scan commit must reset current_index to the latest (first) entry"
        );
        let files = service.get_files().await;
        assert_eq!(files[0].path, newest, "first entry must be the newest by sort_time");
    }

    #[tokio::test]
    async fn add_file_keeps_existing_exif_entry_unchanged() {
        // 语义钉住：对"已存在且带 EXIF"的条目再次 add_file（watcher/FTP 双
        // 通道并发的常态）→ 条目原样保留：sort_time 不变、无重复。这是
        // watcher 事件并发化（file_index/watcher.rs）正确性的根基。
        let temp_dir = tempdir().expect("failed to create temp dir");
        let save_path = temp_dir.path().join("images");
        std::fs::create_dir_all(&save_path).expect("create dir");

        let config_service = ConfigService::new_with_path(temp_dir.path().join("config.json"));
        config_service
            .mutate_and_persist(|config| {
                config.save_path = save_path.clone();
            })
            .expect("persist config");
        let service = FileIndexService::new(Arc::new(config_service));

        let file_path = save_path.join("exif.jpg");
        std::fs::write(&file_path, crate::image_utils::build_exif_jpeg("2024:06:01 12:00:00", 1))
            .expect("write exif jpeg");
        filetime::set_file_mtime(&file_path, filetime::FileTime::from_unix_time(5000, 0))
            .expect("set mtime");

        service.add_file(file_path.clone()).await.expect("first add");
        let before = service.get_files().await;
        assert_eq!(before.len(), 1);
        let sort_time_before = before[0].sort_time;
        let exif_before = before[0].exif_time;
        assert!(exif_before.is_some(), "seed entry must carry EXIF");

        // 即便之后 mtime 拨到远新于原值，已带 EXIF 的条目也不得被重写
        //（重解析会得到不同的 sort_time，可用本断言检出）
        filetime::set_file_mtime(&file_path, filetime::FileTime::from_unix_time(9000, 0))
            .expect("bump mtime");

        service.add_file(file_path.clone()).await.expect("second add");

        let after = service.get_files().await;
        assert_eq!(after.len(), 1, "repeated add must not duplicate the entry");
        assert_eq!(after[0].sort_time, sort_time_before, "sort_time must stay untouched");
        assert_eq!(after[0].exif_time, exif_before, "exif_time must stay untouched");
    }
}
