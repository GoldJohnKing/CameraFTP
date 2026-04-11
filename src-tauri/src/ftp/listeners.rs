// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::constants::FILE_READY_TIMEOUT_SECS;
use crate::file_index::FileIndexService;
use crate::ftp::events::EventBus;
use crate::ftp::stats::StatsActor;
use crate::utils::wait_for_file_ready;
use dashmap::DashSet;
use libunftp::notification::{DataEvent, DataListener, EventMeta, PresenceEvent, PresenceListener};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tracing::{info, warn};

/// 数据事件监听器（上传、下载等）
#[derive(Debug, Clone)]
pub struct FtpDataListener {
    stats: StatsActor,
    event_bus: EventBus,
    save_path: Arc<std::path::PathBuf>,
    app_handle: Option<AppHandle>,
}

impl FtpDataListener {
    pub fn new(stats: StatsActor, event_bus: EventBus, save_path: std::path::PathBuf, app_handle: Option<AppHandle>) -> Self {
        Self { stats, event_bus, save_path: Arc::new(save_path), app_handle }
    }
}

impl DataListener for FtpDataListener {
    fn receive_data_event<'life0, 'async_trait>(
        &'life0 self,
        event: DataEvent,
        _meta: EventMeta,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
    {
        let stats = self.stats.clone();
        let event_bus = self.event_bus.clone();
        let save_path = self.save_path.clone();
        let app_handle = self.app_handle.clone();
        Box::pin(async move {
            match event {
                DataEvent::Put { path, bytes } => {
                    // 记录上传统计
                    stats.record_upload(path.clone(), bytes).await;
                    // 发送文件上传瞬时事件（仅影响已订阅的媒体/前端消费者）
                    event_bus.emit_file_uploaded(path.clone(), bytes);
                    info!(file = %path, size = bytes, "File uploaded");

                    // 检查是否是支持的图片文件
                    let is_image = FileIndexService::is_supported_image(std::path::Path::new(&path));

                    // Windows 平台自动打开图片
                    #[cfg(target_os = "windows")]
                    if let Some(handle) = app_handle.as_ref() {
                        if is_image {
                            let full_path = save_path.join(&path);
                            let handle_clone = handle.clone();
                            // 使用 tokio::spawn 启动异步任务，避免阻塞事件处理
                            tokio::spawn(async move {
                                // 等待文件就绪（而非固定延迟）
                                if wait_for_file_ready(&full_path, Duration::from_secs(FILE_READY_TIMEOUT_SECS)).await {
                                    // 调用 AutoOpenService 处理预览（服务内部会检查 enabled 状态）
                                    let auto_open: tauri::State<'_, crate::auto_open::AutoOpenService> = handle_clone.state();
                                    if let Err(e) = auto_open.on_file_uploaded(full_path.clone()).await {
                                        tracing::error!("Failed to auto open image: {}", e);
                                    }
                                } else {
                                    tracing::warn!("File not ready after timeout: {:?}", full_path);
                                }
                            });
                        } else {
                            info!(file = %path, "Non-image file uploaded, skipping auto-preview");
                        }
                    }

                    // 添加到文件索引（所有平台）
                    if is_image {
                        if let Some(handle) = app_handle.as_ref() {
                            let full_path = save_path.join(&path);
                            let handle_clone = handle.clone();
                            tokio::spawn(async move {
                                // 等待文件就绪（而非固定延迟）
                                if wait_for_file_ready(&full_path, Duration::from_secs(FILE_READY_TIMEOUT_SECS)).await {
                                    if let Some(file_index) = handle_clone.try_state::<Arc<FileIndexService>>() {
                                        if let Err(e) = file_index.add_file(full_path).await {
                                            tracing::warn!("Failed to add file to index: {}", e);
                                        }
                                    }
                                } else {
                                    tracing::warn!("File not ready for indexing after timeout: {:?}", full_path);
                                }
                            });
                        }
                    }
                }
                DataEvent::Got { path, bytes } => {
                    info!(file = %path, size = bytes, "File downloaded");
                    stats.record_download(path, bytes).await;
                }
                DataEvent::Deleted { path } => {
                    stats.record_delete(path.clone()).await;
                    info!(file = %path, "File deleted");

                    let is_image = FileIndexService::is_supported_image(std::path::Path::new(&path));

                    // 从文件索引中移除
                    if let Some(handle) = app_handle.as_ref() {
                        let full_path = save_path.join(&path);
                        let handle_clone = handle.clone();
                        tokio::spawn(async move {
                            if let Some(file_index) = handle_clone.try_state::<Arc<FileIndexService>>() {
                                if let Err(e) = file_index.remove_file(&full_path).await {
                                    tracing::warn!("Failed to remove file from index: {}", e);
                                }
                            }
                        });

                        if is_image {
                            if let Err(err) = handle.emit("media-library-refresh-requested", ()) {
                                warn!(error = %err, file = %path, "Failed to emit media refresh event after delete");
                            }
                        }
                    }
                }
                DataEvent::MadeDir { path } => {
                    info!(dir = %path, "Directory created");
                    stats.record_mkdir(path).await;
                }
                DataEvent::RemovedDir { path } => {
                    info!(dir = %path, "Directory removed");
                    stats.record_rmdir(path).await;
                }
                DataEvent::Renamed { from, to } => {
                    info!(from = %from, to = %to, "File renamed");
                    stats.record_rename(from, to).await;
                }
            }
        })
    }
}

/// 在线状态监听器（登录、登出）
#[derive(Debug, Clone)]
pub struct FtpPresenceListener {
    stats: StatsActor,
    sessions: Arc<DashSet<String>>,
}

impl FtpPresenceListener {
    pub fn new(stats: StatsActor, sessions: Arc<DashSet<String>>) -> Self {
        Self { stats, sessions }
    }
}

impl PresenceListener for FtpPresenceListener {
    fn receive_presence_event<'life0, 'async_trait>(
        &'life0 self,
        event: PresenceEvent,
        meta: EventMeta,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
    {
        let sessions = self.sessions.clone();
        let stats = self.stats.clone();

        Box::pin(async move {
            match event {
                PresenceEvent::LoggedIn => {
                    let is_new = sessions.insert(meta.trace_id.clone());
                    let count = sessions.len() as u64;

                    if is_new {
                        info!(
                            username = %meta.username,
                            trace_id = %meta.trace_id,
                            total_connections = count,
                            "User logged in"
                        );
                    } else {
                        warn!(
                            username = %meta.username,
                            trace_id = %meta.trace_id,
                            "Duplicate LoggedIn event received"
                        );
                    }

                    stats.update_connection_count(count).await;
                }
                PresenceEvent::LoggedOut => {
                    let existed = sessions.remove(&meta.trace_id).is_some();
                    let count = sessions.len() as u64;

                    if existed {
                        info!(
                            username = %meta.username,
                            trace_id = %meta.trace_id,
                            total_connections = count,
                            "User logged out"
                        );
                    } else {
                        warn!(
                            username = %meta.username,
                            trace_id = %meta.trace_id,
                            "LoggedOut for unknown session"
                        );
                    }

                    stats.update_connection_count(count).await;
                }
            }
        })
    }
}
