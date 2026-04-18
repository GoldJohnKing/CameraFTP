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
                    stats.record_upload(path.clone(), bytes).await;
                    event_bus.emit_file_uploaded(path.clone(), bytes);
                    info!(file = %path, size = bytes, "File uploaded");

                    let is_image = FileIndexService::is_supported_image(std::path::Path::new(&path));

                    if is_image {
                        if let Some(handle) = app_handle.as_ref() {
                            let full_path = save_path.join(&path);
                            let handle_clone = handle.clone();
                            tokio::spawn(async move {
                                if !wait_for_file_ready(&full_path, Duration::from_secs(FILE_READY_TIMEOUT_SECS)).await {
                                    tracing::warn!("File not ready after timeout: {:?}", full_path);
                                    return;
                                }

                                // File indexing
                                if let Some(file_index) = handle_clone.try_state::<Arc<FileIndexService>>() {
                                    if let Err(e) = file_index.add_file(full_path.clone()).await {
                                        tracing::warn!("Failed to add file to index: {}", e);
                                    }
                                }

                                // AI edit (all platforms)
                                let ai_edit: tauri::State<'_, crate::ai_edit::AiEditService> = handle_clone.state();
                                ai_edit.on_file_uploaded(full_path.clone()).await;

                                // Auto-open (Windows only)
                                #[cfg(target_os = "windows")]
                                {
                                    let auto_open: tauri::State<'_, crate::auto_open::AutoOpenService> = handle_clone.state();
                                    if let Err(e) = auto_open.on_file_uploaded(full_path).await {
                                        tracing::error!("Failed to auto open image: {}", e);
                                    }
                                }
                                #[cfg(not(target_os = "windows"))]
                                let _ = &full_path; // suppress unused warning
                            });
                        }
                    } else {
                        info!(file = %path, "Non-image file uploaded, skipping auto-preview");
                    }
                }
                DataEvent::Got { path, bytes } => {
                    info!(file = %path, size = bytes, "File downloaded");
                }
                DataEvent::Deleted { path } => {
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
                }
                DataEvent::RemovedDir { path } => {
                    info!(dir = %path, "Directory removed");
                }
                DataEvent::Renamed { from, to } => {
                    info!(from = %from, to = %to, "File renamed");
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
