use crate::ftp::events::EventBus;
use crate::ftp::stats::StatsActor;
use crate::config::AppConfig;
use dashmap::DashSet;
use libunftp::notification::{DataEvent, DataListener, EventMeta, PresenceEvent, PresenceListener};
use std::sync::Arc;
use tracing::{info, warn};

/// 数据事件监听器（上传、下载等）
#[derive(Debug, Clone)]
pub struct FtpDataListener {
    stats: StatsActor,
    event_bus: EventBus,
    save_path: std::path::PathBuf,
}

impl FtpDataListener {
    pub fn new(stats: StatsActor, event_bus: EventBus, save_path: std::path::PathBuf) -> Self {
        Self { stats, event_bus, save_path }
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
        Box::pin(async move {
            match event {
                DataEvent::Put { path, bytes } => {
                    // 记录上传统计
                    stats.record_upload(path.clone(), bytes).await;
                    // 发送文件上传事件（用于Android媒体扫描）
                    event_bus.emit_file_uploaded(path.clone(), bytes);
                    info!(file = %path, size = bytes, "File uploaded");

                    // Windows 平台自动打开图片
                    #[cfg(target_os = "windows")]
                    {
                        let config = AppConfig::load();
                        if config.preview_config.enabled {
                            // 检查是否是图片文件
                            let is_image = path.to_lowercase().ends_with(".jpg")
                                || path.to_lowercase().ends_with(".jpeg")
                                || path.to_lowercase().ends_with(".png")
                                || path.to_lowercase().ends_with(".gif")
                                || path.to_lowercase().ends_with(".bmp")
                                || path.to_lowercase().ends_with(".webp");

                            if is_image {
                                let full_path = save_path.join(&path);
                                // 使用 tokio::spawn 启动异步任务，避免阻塞事件处理
                                tokio::spawn(async move {
                                    // 延迟一小段时间确保文件写入完成
                                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                                    
                                    // 这里通过事件总线通知前端显示预览窗口
                                    // 实际的 AutoOpenService 调用将在前端处理
                                    // 因为监听器没有访问 AppHandle 的权限
                                    info!("Windows auto preview triggered for: {:?}", full_path);
                                });
                            }
                        }
                    }
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
