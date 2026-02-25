use crate::ftp::events::EventBus;
use crate::ftp::stats::StatsActor;
use dashmap::DashSet;
use libunftp::notification::{DataEvent, DataListener, EventMeta, PresenceEvent, PresenceListener};
use std::sync::Arc;
use tracing::{info, warn};

/// 数据事件监听器（上传、下载等）
#[derive(Debug, Clone)]
pub struct FtpDataListener {
    stats: StatsActor,
    event_bus: EventBus,
}

impl FtpDataListener {
    pub fn new(stats: StatsActor, event_bus: EventBus) -> Self {
        Self { stats, event_bus }
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
        Box::pin(async move {
            match event {
                DataEvent::Put { path, bytes } => {
                    // 记录上传统计
                    stats.record_upload(path.clone(), bytes).await;
                    // 发送文件上传事件（用于Android媒体扫描）
                    event_bus.emit_file_uploaded(path.clone(), bytes);
                    info!(file = %path, size = bytes, "File uploaded");
                }
                DataEvent::Got { path, bytes } => {
                    stats.record_download(path.clone(), bytes).await;
                    info!(file = %path, size = bytes, "File downloaded");
                }
                DataEvent::Deleted { path } => {
                    stats.record_delete(path.clone()).await;
                    info!(file = %path, "File deleted");
                }
                DataEvent::MadeDir { path } => {
                    stats.record_mkdir(path.clone()).await;
                    info!(dir = %path, "Directory created");
                }
                DataEvent::RemovedDir { path } => {
                    stats.record_rmdir(path.clone()).await;
                    info!(dir = %path, "Directory removed");
                }
                DataEvent::Renamed { from, to } => {
                    stats.record_rename(from.clone(), to.clone()).await;
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
