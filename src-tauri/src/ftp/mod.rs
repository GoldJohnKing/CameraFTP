use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use libunftp::notification::{DataEvent, DataListener, EventMeta, PresenceEvent, PresenceListener};
use libunftp::options::Shutdown;
use libunftp::ServerBuilder;
use serde::{Deserialize, Serialize};
use tokio::sync::{oneshot, RwLock};
use tracing::{info, warn};

/// FTP 服务器统计数据快照
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ServerStats {
    pub active_connections: u64,
    pub total_uploads: u64,
    pub total_bytes_received: u64,
    pub last_uploaded_file: Option<String>,
}

/// FTP 服务器配置
#[derive(Clone, Debug)]
pub struct ServerConfig {
    pub port: u16,
    pub root_path: PathBuf,
    pub allow_anonymous: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: 21,
            root_path: PathBuf::from("./ftp_root"),
            allow_anonymous: true,
        }
    }
}

/// FTP 服务器状态快照（兼容旧接口）
#[derive(Debug, Clone, serde::Serialize)]
pub struct ServerStateSnapshot {
    pub is_running: bool,
    pub connected_clients: usize,
    pub files_received: u64,
    pub bytes_received: u64,
    pub last_file: Option<String>,
}

/// FTP 服务器
pub struct FtpServer {
    config: ServerConfig,
    shutdown_tx: Option<oneshot::Sender<()>>,
    stats: Arc<RwLock<ServerStats>>,
    sessions: Arc<DashMap<String, ()>>,
    is_running: bool,
}

impl FtpServer {
    pub fn new(config: ServerConfig) -> Self {
        Self {
            config,
            shutdown_tx: None,
            stats: Arc::new(RwLock::new(ServerStats::default())),
            sessions: Arc::new(DashMap::new()),
            is_running: false,
        }
    }

    /// 启动 FTP 服务器
    pub async fn start(&mut self) -> Result<std::net::SocketAddr, Box<dyn std::error::Error + Send + Sync>> {
        if self.is_running {
            return Err("Server is already running".into());
        }

        // 创建关闭信号通道
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        self.shutdown_tx = Some(shutdown_tx);

        // 克隆统计数据用于监听器
        let stats = self.stats.clone();
        let sessions = self.sessions.clone();
        let root_path = self.config.root_path.clone();
        let port = self.config.port;

        // 创建数据监听器
        let data_listener = FtpDataListener {
            stats: stats.clone(),
        };

        // 创建在线状态监听器
        let presence_listener = FtpPresenceListener {
            stats: stats.clone(),
            sessions: sessions.clone(),
        };

        // 创建 FTP 服务器
        let server = ServerBuilder::new(Box::new(move || {
            unftp_sbe_fs::Filesystem::new(root_path.clone()).unwrap()
        }))
        .greeting("Camera FTP Companion Ready")
        .passive_ports(50000..=50100)
        .idle_session_timeout(600) // 10 分钟空闲超时
        .notify_data(data_listener)
        .notify_presence(presence_listener)
        .shutdown_indicator(async move {
            let _ = shutdown_rx.await;
            info!("FTP server shutdown signal received");
            Shutdown::new().grace_period(Duration::from_secs(5))
        })
        .build()?;

        let bind_addr: std::net::SocketAddr = ([0, 0, 0, 0], port).into();
        let actual_addr = bind_addr;

        info!("Starting FTP server on {}", actual_addr);

        // 在后台运行服务器
        let bind_str = bind_addr.to_string();
        tokio::spawn(async move {
            if let Err(e) = server.listen(bind_str).await {
                warn!("FTP server error: {}", e);
            }
            info!("FTP server stopped");
        });

        self.is_running = true;

        Ok(actual_addr)
    }

    /// 停止 FTP 服务器
    pub fn stop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
            info!("FTP server stop signal sent");
        }
        self.is_running = false;
    }

    /// 获取服务器状态快照
    pub fn state_snapshot(&self) -> ServerStateSnapshot {
        // 使用 block_on 可能有问题，但由于这是同步调用，我们需要另一种方式
        // 这里使用 try_read 避免阻塞
        let stats = self.stats.try_read();
        
        match stats {
            Ok(s) => ServerStateSnapshot {
                is_running: self.is_running,
                connected_clients: s.active_connections as usize,
                files_received: s.total_uploads,
                bytes_received: s.total_bytes_received,
                last_file: s.last_uploaded_file.clone(),
            },
            Err(_) => ServerStateSnapshot {
                is_running: self.is_running,
                connected_clients: 0,
                files_received: 0,
                bytes_received: 0,
                last_file: None,
            },
        }
    }

    /// 异步获取统计数据
    pub async fn get_stats(&self) -> ServerStats {
        self.stats.read().await.clone()
    }

    /// 获取配置
    pub fn config(&self) -> &ServerConfig {
        &self.config
    }
}

/// 数据事件监听器（上传、下载等）
#[derive(Debug)]
struct FtpDataListener {
    stats: Arc<RwLock<ServerStats>>,
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
                    let mut s = stats.write().await;
                    s.total_uploads += 1;
                    s.total_bytes_received += bytes;
                    s.last_uploaded_file = Some(path.clone());
                    info!("File uploaded: {} ({} bytes)", path, bytes);
                }
                DataEvent::Got { path, bytes } => {
                    info!("File downloaded: {} ({} bytes)", path, bytes);
                }
                DataEvent::Deleted { path } => {
                    info!("File deleted: {}", path);
                }
                DataEvent::MadeDir { path } => {
                    info!("Directory created: {}", path);
                }
                DataEvent::RemovedDir { path } => {
                    info!("Directory removed: {}", path);
                }
                DataEvent::Renamed { from, to } => {
                    info!("File renamed: {} -> {}", from, to);
                }
            }
        })
    }
}

/// 在线状态监听器（登录、登出）
#[derive(Debug)]
struct FtpPresenceListener {
    stats: Arc<RwLock<ServerStats>>,
    sessions: Arc<DashMap<String, ()>>,
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
        let stats = self.stats.clone();
        let sessions = self.sessions.clone();
        Box::pin(async move {
            match event {
                PresenceEvent::LoggedIn => {
                    sessions.insert(meta.trace_id.clone(), ());
                    let mut s = stats.write().await;
                    s.active_connections = sessions.len() as u64;
                    info!(
                        "User logged in: {} (session: {})",
                        meta.username, meta.trace_id
                    );
                }
                PresenceEvent::LoggedOut => {
                    sessions.remove(&meta.trace_id);
                    let mut s = stats.write().await;
                    s.active_connections = sessions.len() as u64;
                    info!(
                        "User logged out: {} (session: {})",
                        meta.username, meta.trace_id
                    );
                }
            }
        })
    }
}

// 保持兼容旧的导出
pub use {FtpServer as Server, ServerConfig as Config, ServerStateSnapshot as StateSnapshot};
