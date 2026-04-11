// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::ftp::events::EventBus;
use crate::ftp::types::ServerStats;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info};

/// 统计信息Actor命令
#[derive(Debug)]
pub enum StatsCommand {
    RecordUpload { path: String, bytes: u64 },
    RecordDownload { path: String, bytes: u64 },
    RecordDelete { path: String },
    RecordMkdir { path: String },
    RecordRmdir { path: String },
    RecordRename { from: String, to: String },
    UpdateConnectionCount { count: u64 },
}

/// 统计信息Actor句柄
/// 持有共享状态引用，可以直接读取统计
#[derive(Debug, Clone)]
pub struct StatsActor {
    tx: mpsc::Sender<StatsCommand>,
    /// 共享状态引用，用于直接读取（不经过 channel）
    stats: Arc<RwLock<ServerStats>>,
}

impl StatsActor {
    /// 创建带 EventBus 的 StatsActor
    /// 当统计信息变化时，会通过 EventBus 发送 StatsUpdated 事件
    pub fn with_event_bus(event_bus: Option<EventBus>) -> (Self, StatsActorWorker) {
        let (tx, rx) = mpsc::channel(100);
        let stats = Arc::new(RwLock::new(ServerStats::default()));
        let worker = StatsActorWorker::new(rx, stats.clone(), event_bus);
        (Self { tx, stats }, worker)
    }

    /// 直接获取当前统计（从共享状态读取，不经过 channel）
    /// 这是更可靠的方式，避免 channel 竞争问题
    pub async fn get_stats_direct(&self) -> ServerStats {
        self.stats.read().await.clone()
    }

    /// 记录文件上传
    pub async fn record_upload(&self, path: String, bytes: u64) {
        if let Err(e) = self.tx.send(StatsCommand::RecordUpload { path, bytes }).await {
            tracing::warn!("Failed to send record_upload command: {}", e);
        }
    }

    /// 记录文件下载
    pub async fn record_download(&self, path: String, bytes: u64) {
        if let Err(e) = self.tx.send(StatsCommand::RecordDownload { path, bytes }).await {
            tracing::warn!("Failed to send record_download command: {}", e);
        }
    }

    /// 记录文件删除
    pub async fn record_delete(&self, path: String) {
        if let Err(e) = self.tx.send(StatsCommand::RecordDelete { path }).await {
            tracing::warn!("Failed to send record_delete command: {}", e);
        }
    }

    /// 记录目录创建
    pub async fn record_mkdir(&self, path: String) {
        if let Err(e) = self.tx.send(StatsCommand::RecordMkdir { path }).await {
            tracing::warn!("Failed to send record_mkdir command: {}", e);
        }
    }

    /// 记录目录删除
    pub async fn record_rmdir(&self, path: String) {
        if let Err(e) = self.tx.send(StatsCommand::RecordRmdir { path }).await {
            tracing::warn!("Failed to send record_rmdir command: {}", e);
        }
    }

    /// 记录文件重命名
    pub async fn record_rename(&self, from: String, to: String) {
        if let Err(e) = self.tx.send(StatsCommand::RecordRename { from, to }).await {
            tracing::warn!("Failed to send record_rename command: {}", e);
        }
    }

    /// 更新连接数
    pub async fn update_connection_count(&self, count: u64) {
        if let Err(e) = self.tx.send(StatsCommand::UpdateConnectionCount { count }).await {
            tracing::warn!("Failed to send update_connection_count command: {}", e);
        }
    }
}

/// 统计信息Actor工作者
pub struct StatsActorWorker {
    rx: mpsc::Receiver<StatsCommand>,
    stats: Arc<RwLock<ServerStats>>,
    /// 事件总线（可选），用于在统计变化时发送事件
    event_bus: Option<EventBus>,
}

impl StatsActorWorker {
    fn new(
        rx: mpsc::Receiver<StatsCommand>,
        stats: Arc<RwLock<ServerStats>>,
        event_bus: Option<EventBus>,
    ) -> Self {
        Self { rx, stats, event_bus }
    }

    /// 运行Actor主循环
    pub async fn run(mut self) {
        while let Some(cmd) = self.rx.recv().await {
            let should_emit = match cmd {
                StatsCommand::RecordUpload { path, bytes } => {
                    let mut stats = self.stats.write().await;
                    stats.total_uploads += 1;
                    stats.total_bytes_received += bytes;
                    stats.last_uploaded_file = Some(path.clone());
                    info!(file = %path, size = bytes, "File uploaded");
                    true // 上传需要发送事件
                }
                StatsCommand::UpdateConnectionCount { count } => {
                    let mut stats = self.stats.write().await;
                    stats.active_connections = count;
                    true // 连接数变化需要发送事件
                }
                StatsCommand::RecordDownload { path, bytes } => {
                    debug!(file = %path, size = bytes, "File downloaded");
                    false
                }
                StatsCommand::RecordDelete { path } => {
                    debug!(file = %path, "File deleted");
                    false
                }
                StatsCommand::RecordMkdir { path } => {
                    debug!(dir = %path, "Directory created");
                    false
                }
                StatsCommand::RecordRmdir { path } => {
                    debug!(dir = %path, "Directory removed");
                    false
                }
                StatsCommand::RecordRename { from, to } => {
                    debug!(from = %from, to = %to, "File renamed");
                    false
                }
            };

            // 如果状态有变化且配置了 EventBus，发送 StatsUpdated 事件
            if should_emit {
                if let Some(ref bus) = self.event_bus {
                    let stats = self.stats.read().await.clone();
                    bus.emit_stats_updated(stats).await;
                }
            }
        }
    }
}
