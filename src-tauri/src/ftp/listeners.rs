use crate::ftp::stats::StatsActor;
use crate::ftp::types::ServerStats;
use dashmap::DashSet;
use libunftp::notification::{DataEvent, DataListener, EventMeta, PresenceEvent, PresenceListener};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// 数据事件监听器（上传、下载等）
#[derive(Debug, Clone)]
pub struct FtpDataListener {
    stats: StatsActor,
}

impl FtpDataListener {
    pub fn new(stats: StatsActor) -> Self {
        Self { stats }
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
        Box::pin(async move {
            match event {
                DataEvent::Put { path, bytes } => {
                    stats.record_upload(path.clone(), bytes).await;
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

    /// 获取当前会话数量
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// 获取所有会话ID
    pub fn session_ids(&self) -> Vec<String> {
        self.sessions.iter().map(|s| s.clone()).collect()
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

/// 传统监听器实现（兼容旧代码）
#[derive(Debug)]
pub struct LegacyDataListener {
    stats: Arc<RwLock<ServerStats>>,
}

impl LegacyDataListener {
    pub fn new(stats: Arc<RwLock<ServerStats>>) -> Self {
        Self { stats }
    }
}

impl DataListener for LegacyDataListener {
    fn receive_data_event<'life0, 'async_trait>(
        &'life0 self,
        event: DataEvent,
        _meta: EventMeta,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
    {
        let stats = self.stats.clone();
        Box::pin(async move {
            match event {
                DataEvent::Put { path, bytes } => {
                    let mut s = stats.write().await;
                    s.total_uploads += 1;
                    s.total_bytes_received += bytes;
                    s.last_uploaded_file = Some(path.clone());
                    info!(file = %path, size = bytes, "File uploaded");
                }
                DataEvent::Got { path, bytes } => {
                    info!(file = %path, size = bytes, "File downloaded");
                }
                DataEvent::Deleted { path } => {
                    info!(file = %path, "File deleted");
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

/// 传统在线状态监听器
#[derive(Debug)]
pub struct LegacyPresenceListener {
    stats: Arc<RwLock<ServerStats>>,
    sessions: Arc<DashSet<String>>,
}

impl LegacyPresenceListener {
    pub fn new(stats: Arc<RwLock<ServerStats>>, sessions: Arc<DashSet<String>>) -> Self {
        Self { stats, sessions }
    }
}

impl PresenceListener for LegacyPresenceListener {
    fn receive_presence_event<'life0, 'async_trait>(
        &'life0 self,
        event: PresenceEvent,
        meta: EventMeta,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
    {
        let stats = self.stats.clone();
        let sessions = self.sessions.clone();

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
                            "Duplicate LoggedIn event"
                        );
                    }

                    let mut s = stats.write().await;
                    s.active_connections = count;
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

                    let mut s = stats.write().await;
                    s.active_connections = count;
                }
            }
        })
    }
}
