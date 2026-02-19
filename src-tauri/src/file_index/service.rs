// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, error, info, trace, warn};

use crate::config::AppConfig;
use crate::error::AppError;
use crate::ftp::EventBus;
use super::types::{FileIndex, FileInfo};
use super::watcher::FileWatcher;

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub struct FileIndexService {
    index: RwLock<FileIndex>,
    save_path: RwLock<PathBuf>,
    watcher: Mutex<Option<FileWatcher>>,
    // 使用 Arc<RwLock<...>> 使 event_bus 可以在克隆实例间共享
    event_bus: Arc<RwLock<Option<EventBus>>>,
}

impl Clone for FileIndexService {
    fn clone(&self) -> Self {
        Self {
            index: RwLock::new(self.index.blocking_read().clone()),
            save_path: RwLock::new(self.save_path.blocking_read().clone()),
            watcher: Mutex::new(None), // watcher 不克隆，新实例需要重新启动
            event_bus: Arc::clone(&self.event_bus), // 共享 event_bus
        }
    }
}

impl FileIndexService {
    pub fn new() -> Self {
        let config = AppConfig::load();
        Self {
            index: RwLock::new(FileIndex::new()),
            save_path: RwLock::new(config.save_path.clone()),
            watcher: Mutex::new(Some(FileWatcher::new(config.save_path))),
            event_bus: Arc::new(RwLock::new(None)),
        }
    }

    /// 设置事件总线
    pub async fn set_event_bus(&self, event_bus: EventBus) {
        *self.event_bus.write().await = Some(event_bus);
    }

    /// 发射文件索引变化事件（异步版本，确保事件可靠发射）
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
                count = index.files.len();
                latest_filename = index.files.first().map(|f| f.filename.clone());
            }
            trace!("File index changed event emitted: count={}, latest={:?}", count, latest_filename);
            event_bus.emit_file_index_changed(count, latest_filename);
        }
    }

    /// 启动文件系统监听（桌面平台）
    /// 注意：需要传入 Arc<Self> 以在 watcher 任务中保持服务存活
    pub async fn start_watcher(self_arc: Arc<Self>) -> Result<bool, AppError> {
        #[cfg(not(target_os = "android"))]
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

                let result = match watcher.start(self_arc_clone).await {
                    Ok(true) => {
                        info!("File watcher started successfully");
                        // 将 watcher 重新放回 Mutex
                        let mut watcher_guard = self_arc.watcher.lock().await;
                        *watcher_guard = Some(watcher);
                        Ok(true)
                    }
                    Ok(false) => {
                        info!("File watcher not started (may be unsupported platform)");
                        // 将 watcher 重新放回 Mutex
                        let mut watcher_guard = self_arc.watcher.lock().await;
                        *watcher_guard = Some(watcher);
                        Ok(false)
                    }
                    Err(e) => {
                        error!("Failed to start file watcher: {}", e);
                        // 将 watcher 重新放回 Mutex
                        let mut watcher_guard = self_arc.watcher.lock().await;
                        *watcher_guard = Some(watcher);
                        Err(AppError::Other(format!("Failed to start watcher: {}", e)))
                    }
                };
                result
            } else {
                Ok(false)
            }
        }

        #[cfg(target_os = "android")]
        {
            // Android 使用 FileObserver，在 Kotlin 侧实现
            info!("File watcher on Android is handled by FileObserverBridge");
            Ok(false)
        }
    }

    /// 停止文件系统监听
    pub async fn stop_watcher(&self) {
        let mut watcher_guard = self.watcher.lock().await;
        if let Some(ref mut watcher) = *watcher_guard {
            watcher.stop();
            info!("File watcher stopped");
        }
    }

    /// 处理来自 Android 的文件创建事件
    pub async fn handle_external_created(&self, path: PathBuf) {
        info!("Handling external file creation: {:?}", path);

        // 延迟处理确保文件写入完成
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        if let Err(e) = self.add_file(path.clone()).await {
            warn!("Failed to add external file to index: {}", e);
        } else {
            info!("External file added to index: {:?}", path);
        }
    }

    /// 处理来自 Android 的文件删除事件
    pub async fn handle_external_deleted(&self, path: PathBuf) {
        info!("Handling external file deletion: {:?}", path);

        match self.remove_file(&path).await {
            Ok(true) => {
                info!("External file removed from index: {:?}", path);
            }
            Ok(false) => {
                // 文件不在索引中，忽略（幂等性保证）
                debug!("External file not in index, ignoring: {:?}", path);
            }
            Err(e) => {
                error!("Failed to remove external file from index: {}", e);
            }
        }
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
        index.files = files;
        index.current_index = index.files.first().map(|_| 0);
        
        info!("Directory scan complete: {} files found", index.files.len());
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
                
                if metadata.is_err() {
                    continue; // 跳过无权限文件
                }
                let metadata = metadata.unwrap();
                
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
        
        Ok(FileInfo {
            path: path.to_path_buf(),
            filename,
            exif_time,
            modified_time,
            sort_time,
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

        let mut index = self.index.write().await;

        // 插入到正确位置（保持排序：新→旧）
        let insert_pos = index.files.iter()
            .position(|f| f.sort_time < file_info.sort_time)
            .unwrap_or(index.files.len());

        index.files.insert(insert_pos, file_info);

        // 更新 current_index 如果插入位置在 current_index 之前
        if let Some(current) = index.current_index {
            if insert_pos <= current {
                index.current_index = Some(current + 1);
            }
        }

        drop(index);
        info!("Added file to index: {:?}", path);

        // 发射文件索引变化事件
        self.emit_file_index_changed().await;

        Ok(())
    }

    /// 从索引中移除文件
    pub async fn remove_file(&self, path: &Path) -> Result<bool, AppError> {
        let mut index = self.index.write().await;

        if let Some(pos) = index.files.iter().position(|f| f.path == path) {
            index.files.remove(pos);

            // 调整 current_index
            if let Some(current) = index.current_index {
                if pos < current {
                    // 删除在当前位置之前，索引减1
                    index.current_index = Some(current - 1);
                } else if pos == current {
                    // 删除的是当前文件，尝试保持有效索引
                    if index.files.is_empty() {
                        index.current_index = None;
                    } else if current >= index.files.len() {
                        index.current_index = Some(index.files.len() - 1);
                    }
                    // 否则保持 current 不变（指向下一个文件）
                }
            }

            drop(index);
            info!("Removed file from index: {:?}", path);

            // 发射文件索引变化事件
            self.emit_file_index_changed().await;

            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// 检查并清理索引中不存在的文件
    /// 返回清理的文件数量和新的当前索引
    pub async fn cleanup_missing_files(&self) -> Result<(usize, Option<usize>), AppError> {
        let mut index = self.index.write().await;
        let _original_len = index.files.len();
        let original_current = index.current_index;

        // 找出不存在的文件
        let mut missing_positions = Vec::new();
        for (pos, file) in index.files.iter().enumerate() {
            if !tokio::fs::try_exists(&file.path).await.unwrap_or(false) {
                missing_positions.push(pos);
            }
        }

        if missing_positions.is_empty() {
            return Ok((0, original_current));
        }

        // 从后向前删除，避免索引偏移问题
        for &pos in missing_positions.iter().rev() {
            index.files.remove(pos);
            info!("Cleaned up missing file from index: position {}", pos);
        }

        // 重新计算 current_index
        let new_current = if index.files.is_empty() {
            None
        } else if let Some(current) = original_current {
            // 统计在当前位置之前删除了多少个文件
            let removed_before = missing_positions.iter().filter(|&&p| p < current).count();
            let new_pos = current.saturating_sub(removed_before);
            Some(new_pos.min(index.files.len() - 1))
        } else {
            Some(0)
        };

        index.current_index = new_current;

        let cleaned_count = missing_positions.len();
        info!("Cleanup complete: removed {} files, current index: {:?}", cleaned_count, new_current);
        Ok((cleaned_count, new_current))
    }

    /// 获取文件列表
    pub async fn get_files(&self) -> Arc<Vec<FileInfo>> {
        let index = self.index.read().await;
        Arc::new(index.files.clone())
    }

    /// 获取当前索引
    pub async fn get_current_index(&self) -> Option<usize> {
        let index = self.index.read().await;
        index.current_index
    }

    /// 导航到指定索引
    /// 如果目标文件不存在，会尝试清理并返回错误
    pub async fn navigate_to(&self, new_index: usize) -> Result<FileInfo, AppError> {
        // 先验证索引范围并获取文件信息
        let file_info = {
            let index = self.index.read().await;
            if new_index >= index.files.len() {
                return Err(AppError::Other("Index out of bounds".to_string()));
            }
            index.files[new_index].clone()
        }; // 读锁在这里自动释放

        // 检查文件是否存在
        if !tokio::fs::try_exists(&file_info.path).await.unwrap_or(false) {
            // 文件不存在，清理索引
            let mut index = self.index.write().await;
            if let Some(pos) = index.files.iter().position(|f| f.path == file_info.path) {
                index.files.remove(pos);
                // 调整 current_index
                if let Some(current) = index.current_index {
                    if pos < current {
                        index.current_index = Some(current - 1);
                    } else if pos == current && current >= index.files.len() && !index.files.is_empty() {
                        index.current_index = Some(index.files.len() - 1);
                    }
                }
                info!("Removed missing file from index during navigation: {:?}", file_info.path);
            }
            return Err(AppError::Other(format!("File not found: {}", file_info.path.display())));
        }

        // 然后更新当前索引
        let mut index = self.index.write().await;
        index.current_index = Some(new_index);

        Ok(file_info)
    }

    /// 获取最新文件（排序第一个）
    pub async fn get_latest_file(&self) -> Option<FileInfo> {
        let index = self.index.read().await;
        index.files.first().cloned()
    }

    /// 根据文件路径查找索引
    pub async fn find_file_index(&self, path: &Path) -> Option<usize> {
        let index = self.index.read().await;
        index.files.iter().position(|f| f.path == path)
    }

    /// 获取文件数量
    #[allow(dead_code)]
    pub async fn get_file_count(&self) -> usize {
        let index = self.index.read().await;
        index.files.len()
    }

    /// 更新存储路径并重新扫描
    pub async fn update_save_path(&self, new_path: PathBuf) -> Result<(), AppError> {
        let current_path = self.save_path.read().await.clone();
        if current_path != new_path {
            info!("Updating save_path from {:?} to {:?}", current_path, new_path);

            // 停止当前 watcher
            self.stop_watcher().await;

            // 更新路径
            *self.save_path.write().await = new_path.clone();

            // 重新扫描
            self.scan_directory().await?;

            // 重启 watcher（桌面平台）
            #[cfg(not(target_os = "android"))]
            {
                let mut watcher_guard = self.watcher.lock().await;
                *watcher_guard = Some(FileWatcher::new(new_path));
            }
        }
        Ok(())
    }

    /// 触发文件系统事件（供 Android FileObserver 调用）
    pub async fn notify_file_event(&self, event_type: &str, path: PathBuf) {
        match event_type {
            "created" => self.handle_external_created(path).await,
            "deleted" => self.handle_external_deleted(path).await,
            _ => {
                warn!("Unknown file event type: {}", event_type);
            }
        }
    }
}

impl Default for FileIndexService {
    fn default() -> Self {
        Self::new()
    }
}
