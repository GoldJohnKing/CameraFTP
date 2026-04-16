// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::RwLock;
#[cfg(target_os = "windows")]
use tokio::sync::Mutex;
use tracing::{info, trace, warn};
#[cfg(target_os = "windows")]
use tracing::error;

use crate::config::AppConfig;
use crate::config_service::ConfigService;
use crate::error::AppError;
use crate::ftp::EventBus;
use super::types::{FileIndex, FileInfo};
#[cfg(target_os = "windows")]
use super::watcher::FileWatcher;

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub struct FileIndexService {
    index: RwLock<FileIndex>,
    save_path: RwLock<PathBuf>,
    #[cfg(target_os = "windows")]
    watcher: Mutex<Option<FileWatcher>>,
    // 使用 Arc<RwLock<...>> 使 event_bus 可以在克隆实例间共享
    event_bus: Arc<RwLock<Option<EventBus>>>,
}

impl FileIndexService {
    pub fn new(config_service: Arc<ConfigService>) -> Self {
        let config = config_service.get().unwrap_or_else(|e| {
            warn!(error = %e, "Failed to read config from ConfigService, using defaults");
            AppConfig::default()
        });
        Self {
            index: RwLock::new(FileIndex::new()),
            save_path: RwLock::new(config.save_path.clone()),
            #[cfg(target_os = "windows")]
            watcher: Mutex::new(Some(FileWatcher::new(config.save_path))),
            event_bus: Arc::new(RwLock::new(None)),
        }
    }

    /// 设置事件总线
    pub async fn set_event_bus(&self, event_bus: EventBus) {
        *self.event_bus.write().await = Some(event_bus);
    }

    /// 发射文件索引变化事件（仅投递给已订阅的瞬时消费者）
    async fn emit_file_index_changed(&self) {
        // 获取 event_bus（使用阻塞锁确保获取成功）
        let event_bus_opt = {
            let guard = self.event_bus.read().await;
            guard.clone()
        };

        if let Some(ref event_bus) = event_bus_opt {
            // 获取索引信息
            let count;
            let latest_filename;
            {
                let index = self.index.read().await;
                count = index.files().len();
                latest_filename = index.files().first().map(|f| f.filename.clone());
            }
            trace!("File index changed event emitted: count={}, latest={:?}", count, latest_filename);
            event_bus.emit_file_index_changed(count, latest_filename);
        }
    }

    /// 启动文件系统监听（桌面平台）
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

            // 重新获取 watcher 的可变引用并启动
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
        if let Some(ref mut watcher) = *watcher_guard {
            watcher.stop();
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
        
        let mut files = Vec::new();
        self.scan_recursive(&save_path, &mut files).await?;
        
        // 按 sort_time 排序（新→旧，最新的排在最前面）
        files.sort_by(|a, b| b.sort_time.cmp(&a.sort_time));
        
        let mut index = self.index.write().await;
        index.current_index = files.first().map(|_| 0);
        let count = files.len();
        index.set_files(files);
        
        info!("Directory scan complete: {} files found", count);

        drop(index);
        self.emit_file_index_changed().await;

        Ok(())
    }

    /// 递归扫描目录
    fn scan_recursive<'a>(&'a self, dir: &'a Path, files: &'a mut Vec<FileInfo>) -> BoxFuture<'a, Result<(), AppError>> {
        Box::pin(async move {
            let mut entries = tokio::fs::read_dir(dir).await
                .map_err(|e| AppError::Other(format!("Failed to read dir: {}", e)))?;

            while let Some(entry) = entries.next_entry().await
                .map_err(|e| AppError::Other(format!("Failed to read entry: {}", e)))? 
            {
                let path = entry.path();
                let metadata = entry.metadata().await;
                
                let metadata = match metadata {
                    Ok(m) => m,
                    Err(_) => continue, // 跳过无权限文件
                };
                
                if metadata.is_dir() {
                    // 递归扫描子目录
                    if let Err(e) = self.scan_recursive(&path, files).await {
                        warn!("Failed to scan subdirectory {:?}: {}", path, e);
                    }
                } else if metadata.is_file() {
                    // 检查是否是支持的图片格式
                    if Self::is_supported_image(&path) {
                        match self.get_file_info(&path, &metadata).await {
                            Ok(file_info) => files.push(file_info),
                            Err(e) => warn!("Failed to get file info for {:?}: {}", path, e),
                        }
                    }
                }
            }
            
            Ok(())
        })
    }

    /// 检查文件是否是支持的图片格式
    pub fn is_supported_image(path: &Path) -> bool {
        let ext = path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();
        
        matches!(ext.as_str(), "jpg" | "jpeg" | "heif" | "hif" | "heic")
    }

    /// 获取文件信息（包括EXIF时间）
    async fn get_file_info(&self, path: &Path, metadata: &std::fs::Metadata) -> Result<FileInfo, AppError> {
        let filename = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        
        let modified_time = metadata.modified()
            .unwrap_or_else(|_| SystemTime::UNIX_EPOCH);
        
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
        // 使用 spawn_blocking 因为 EXIF 读取是同步操作
        let path = path.to_path_buf();
        tokio::task::spawn_blocking(move || {
            use nom_exif::*;

            let mut parser = MediaParser::new();
            let ms = MediaSource::file_path(&path).ok()?;

            if !ms.has_exif() {
                return None;
            }

            let iter: ExifIter = parser.parse(ms).ok()?;
            let exif: Exif = iter.into();

            // 优先读取 DateTimeOriginal
            let datetime = exif
                .get(ExifTag::DateTimeOriginal)
                .and_then(|v| v.as_time_components())
                .map(|(ndt, _offset)| ndt)?;

            // 转换为 SystemTime - 使用 chrono 的正确转换方法
            datetime.and_utc().try_into().ok()
        }).await.ok()?
    }

    /// 添加新文件（FTP上传时调用）
    pub async fn add_file(&self, path: PathBuf) -> Result<(), AppError> {
        if !Self::is_supported_image(&path) {
            return Ok(()); // 跳过非图片文件
        }

        let metadata = tokio::fs::metadata(&path).await
            .map_err(|e| AppError::Other(format!("Failed to get metadata: {}", e)))?;

        let file_info = self.get_file_info(&path, &metadata).await?;

        // Atomic check-and-insert under write lock to prevent TOCTOU race
        let mut index = self.index.write().await;

        if index.files().iter().any(|f| f.path == path) {
            trace!("File already indexed, skipping: {:?}", path);
            return Ok(());
        }

        // 插入到正确位置（保持排序：新→旧）
        let mut files = index.files().as_ref().clone();
        let insert_pos = files.iter()
            .position(|f| f.sort_time < file_info.sort_time)
            .unwrap_or(files.len());

        files.insert(insert_pos, file_info);

        // 更新 current_index 如果插入位置在 current_index 之前
        if let Some(current) = index.current_index {
            if insert_pos <= current {
                index.current_index = Some(current + 1);
            }
        }

        index.set_files(files);
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
            let mut files = index.files().as_ref().clone();
            files.remove(pos);

            // 调整 current_index
            if let Some(current) = index.current_index {
                if pos < current {
                    // 删除在当前位置之前，索引减1
                    index.current_index = Some(current - 1);
                } else if pos == current {
                    // 删除的是当前文件，尝试保持有效索引
                    if files.is_empty() {
                        index.current_index = None;
                    } else if current >= files.len() {
                        index.current_index = Some(files.len() - 1);
                    }
                    // 否则保持 current 不变（指向下一个文件）
                }
            }

            index.set_files(files);
            drop(index);
            info!("Removed file from index: {:?}", path);

            // 发射文件索引变化事件
            self.emit_file_index_changed().await;

            Ok(true)
        } else {
            Ok(false)
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

        let mut files = index.files().as_ref().clone();
        files.remove(pos);
        let new_len = files.len();
        Self::adjust_current_index_after_removal(&mut index.current_index, pos, new_len);
        index.set_files(files);
        info!("Removed missing file from index: {:?}", path);
    }

    /// 移除文件后调整当前索引
    fn adjust_current_index_after_removal(
        current_index: &mut Option<usize>,
        removed_pos: usize,
        new_len: usize,
    ) {
        let Some(current) = current_index else { return };

        if removed_pos < *current {
            *current_index = Some(*current - 1);
        } else if removed_pos == *current && *current >= new_len && new_len > 0 {
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

    /// 更新存储路径并重新扫描
    pub async fn update_save_path(&self, new_path: PathBuf) -> Result<(), AppError> {
        let current_path = self.save_path.read().await.clone();
        if current_path == new_path {
            return Ok(());
        }

        info!(
            "Updating save_path from {:?} to {:?}",
            current_path, new_path
        );

        self.stop_watcher().await;
        *self.save_path.write().await = new_path.clone();
        self.scan_directory().await?;
        self.restart_watcher(new_path).await;

        Ok(())
    }

    /// 重启文件监听器（桌面平台）
    #[cfg(target_os = "windows")]
    async fn restart_watcher(&self, path: PathBuf) {
        let mut watcher_guard = self.watcher.lock().await;
        *watcher_guard = Some(FileWatcher::new(path));
    }

    /// 重启文件监听器（Android 平台 - 无操作）
    #[cfg(target_os = "android")]
    async fn restart_watcher(&self, _path: PathBuf) {
        // Android 不使用文件系统监听
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use tempfile::tempdir;

    use crate::config_service::ConfigService;

    use super::FileIndexService;

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
}
