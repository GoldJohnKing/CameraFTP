// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

// 文件系统监听仅在 Windows 平台启用
// Android 不使用文件系统监听
#![cfg(target_os = "windows")]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use notify::{Event, RecursiveMode, Watcher, RecommendedWatcher};
use tokio::sync::mpsc::{channel, Sender};
use tracing::{info, debug, error, warn};

use crate::file_index::FileIndexService;
use crate::constants::FILE_READY_TIMEOUT_SECS;
use crate::utils::wait_for_file_ready;

/// 文件系统事件类型
#[derive(Debug, Clone)]
pub enum FileSystemEvent {
    /// 文件创建
    Created(PathBuf),
    /// 文件删除
    Deleted(PathBuf),
    /// 文件重命名
    Renamed { from: PathBuf, to: PathBuf },
}

/// Windows 文件系统监听器
/// 
/// 使用 notify crate 的 ReadDirectoryChangesW 后端
pub struct FileWatcher {
    watcher: Option<RecommendedWatcher>,
    watch_path: PathBuf,
    event_sender: Option<Sender<FileSystemEvent>>,
}

impl FileWatcher {
    /// 创建新的文件监听器
    pub fn new(watch_path: PathBuf) -> Self {
        Self {
            watcher: None,
            watch_path,
            event_sender: None,
        }
    }

    /// 开始监听文件系统事件
    /// 
    /// # Arguments
    /// * `file_index` - 文件索引服务，用于同步索引
    /// 
    /// # Platform Support
    /// - Windows: 使用 notify crate
    pub async fn start(&mut self, file_index: Arc<FileIndexService>) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        if self.watcher.is_some() {
            info!("File watcher already running");
            return Ok(true);
        }

        let (tx, mut rx) = channel::<FileSystemEvent>(100);
        self.event_sender = Some(tx.clone());

        // 创建 notify watcher
        let watcher_tx = tx.clone();
        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                match res {
                    Ok(event) => {
                        Self::handle_notify_event(event, &watcher_tx);
                    }
                    Err(e) => {
                        error!("File watcher error: {}", e);
                    }
                }
            },
            notify::Config::default()
                .with_poll_interval(Duration::from_secs(2))
                .with_compare_contents(true),
        )?;

        // 开始监听
        watcher.watch(&self.watch_path, RecursiveMode::Recursive)?;
        self.watcher = Some(watcher);

        info!("File watcher started for: {:?}", self.watch_path);

        // 启动事件处理任务
        let file_index_clone = file_index.clone();
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                Self::process_event(event, file_index_clone.clone()).await;
            }
        });

        Ok(true)
    }

    /// 停止监听
    pub fn stop(&mut self) {
        if let Some(watcher) = self.watcher.take() {
            drop(watcher);
            info!("File watcher stopped");
        }
        self.event_sender = None;
    }

    /// 处理 notify 事件，转换为内部事件格式
    fn handle_notify_event(event: Event, tx: &Sender<FileSystemEvent>) {
        use notify::EventKind;

        debug!("Raw notify event: {:?}", event);

        match event.kind {
            EventKind::Create(_) => {
                for path in &event.paths {
                    if FileIndexService::is_supported_image(path) {
                        let _ = tx.try_send(FileSystemEvent::Created(path.clone()));
                    }
                }
            }
            EventKind::Modify(_) => {
                // 修改事件不需要索引更新
            }
            EventKind::Remove(_) => {
                for path in &event.paths {
                    if FileIndexService::is_supported_image(path) {
                        let _ = tx.try_send(FileSystemEvent::Deleted(path.clone()));
                    }
                }
            }
            _ => {
                // 其他事件类型（如重命名）
                if event.paths.len() == 2 {
                    // 可能是重命名事件
                    let from = &event.paths[0];
                    let to = &event.paths[1];
                    if FileIndexService::is_supported_image(from) || 
                       FileIndexService::is_supported_image(to) {
                        let _ = tx.try_send(FileSystemEvent::Renamed {
                            from: from.clone(),
                            to: to.clone(),
                        });
                    }
                }
            }
        }
    }

    /// 处理文件系统事件并同步到索引
    async fn process_event(event: FileSystemEvent, file_index: Arc<FileIndexService>) {
        match event {
            FileSystemEvent::Created(path) => {
                debug!("File created: {:?}", path);
                // 等待文件就绪（而非固定延迟）
                if wait_for_file_ready(&path, Duration::from_secs(FILE_READY_TIMEOUT_SECS)).await {
                    if let Err(e) = file_index.add_file(path.clone()).await {
                        warn!("Failed to add file to index: {}", e);
                    } else {
                        info!("File added to index via watcher: {:?}", path);
                    }
                } else {
                    warn!("File not ready after timeout: {:?}", path);
                }
            }
            FileSystemEvent::Deleted(path) => {
                debug!("File deleted: {:?}", path);
                
                match file_index.remove_file(&path).await {
                    Ok(true) => {
                        info!("File removed from index via watcher: {:?}", path);
                    }
                    Ok(false) => {
                        // 文件不在索引中，忽略（幂等性保证）
                        debug!("File not in index, ignoring: {:?}", path);
                    }
                    Err(e) => {
                        error!("Failed to remove file from index: {}", e);
                    }
                }
            }
            FileSystemEvent::Renamed { from, to } => {
                debug!("File renamed: {:?} -> {:?}", from, to);
                
                // 先移除旧路径
                match file_index.remove_file(&from).await {
                    Ok(true) => {
                        info!("Removed old path from index: {:?}", from);
                    }
                    _ => {}
                }
                
                // 等待新路径文件就绪（而非固定延迟）
                if wait_for_file_ready(&to, Duration::from_secs(FILE_READY_TIMEOUT_SECS)).await {
                    if let Err(e) = file_index.add_file(to.clone()).await {
                        warn!("Failed to add renamed file to index: {}", e);
                    } else {
                        info!("Added renamed file to index: {:?}", to);
                    }
                } else {
                    warn!("Renamed file not ready after timeout: {:?}", to);
                }
            }
        }
    }
}

impl Drop for FileWatcher {
    fn drop(&mut self) {
        if self.watcher.take().is_some() {
            tracing::info!("File watcher stopped");
        }
    }
}

