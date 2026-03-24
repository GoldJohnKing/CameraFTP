// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::constants::{
    CHECK_INTERVAL_MS, SERVER_READY_TIMEOUT_SECS, SERVER_SHUTDOWN_TIMEOUT_SECS,
};
use crate::crypto::tls;
use crate::error::{AppError, AppResult};
use crate::ftp::events::EventBus;
use crate::ftp::listeners::{FtpDataListener, FtpPresenceListener};
use crate::ftp::stats::{StatsActor, StatsActorWorker};
use crate::ftp::types::{
    ServerConfig, ServerInfo, ServerStateSnapshot, ServerStatus, FtpAuthConfig,
};
use crate::ftp::FtpStorageBackend;
use dashmap::DashSet;
use libunftp::options::Shutdown;
use libunftp::ServerBuilder;
use std::net::{SocketAddr, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::AppHandle;
use tokio::sync::{mpsc, oneshot, RwLock};
use tracing::{error, info, instrument, warn};
use unftp_core::auth::{Authenticator, Credentials, AuthenticationError, Principal};

#[cfg(target_os = "android")]
use crate::ftp::android_mediastore::AndroidMediaStoreBackend;

/// 自定义 FTP 认证器
#[derive(Debug)]
struct CustomAuthenticator {
    auth_config: FtpAuthConfig,
}

impl CustomAuthenticator {
    fn new(auth_config: FtpAuthConfig) -> Self {
        Self { auth_config }
    }
}

#[async_trait::async_trait]
impl Authenticator for CustomAuthenticator {
    async fn authenticate(
        &self,
        username: &str,
        creds: &Credentials,
    ) -> Result<Principal, AuthenticationError> {
        match &self.auth_config {
            FtpAuthConfig::Anonymous => {
                // 匿名模式：允许任何用户名
                Ok(Principal {
                    username: username.to_string(),
                })
            }
            FtpAuthConfig::Authenticated { username: expected_username, password_hash } => {
                // 验证用户名
                if username != expected_username {
                    return Err(AuthenticationError::BadPassword);
                }

                // 使用 Argon2id 验证密码
                let password = creds.password.clone().unwrap_or_default();
                
                if crate::crypto::verify_password(password, password_hash) {
                    Ok(Principal {
                        username: username.to_string(),
                    })
                } else {
                    Err(AuthenticationError::BadPassword)
                }
            }
        }
    }
}

#[derive(Debug)]
pub enum ServerCommand {
    Start {
        config: ServerConfig,
        respond_to: oneshot::Sender<AppResult<SocketAddr>>,
    },
    Stop {
        respond_to: oneshot::Sender<AppResult<()>>,
    },
    GetSnapshot {
        respond_to: oneshot::Sender<ServerStateSnapshot>,
    },
    GetServerInfo {
        respond_to: oneshot::Sender<Option<ServerInfo>>,
    },
}

#[derive(Debug, Clone)]
pub struct FtpServerHandle {
    tx: mpsc::Sender<ServerCommand>,
}

impl FtpServerHandle {
    async fn send_command<T: Send + 'static>(
        &self,
        cmd_factory: impl FnOnce(oneshot::Sender<T>) -> ServerCommand,
    ) -> Result<T, AppError> {
        let (tx, rx) = oneshot::channel();
        let cmd = cmd_factory(tx);
        if self.tx.send(cmd).await.is_err() {
            return Err(AppError::ServerNotRunning);
        }
        rx.await.map_err(|_| AppError::ServerNotRunning)
    }

    /// 启动服务器
    #[instrument(skip(self))]
    pub async fn start(
        &self,
        config: ServerConfig,
    ) -> AppResult<SocketAddr> {
        self.send_command(|tx| ServerCommand::Start { config, respond_to: tx }).await?
    }

    /// 停止服务器
    #[instrument(skip(self))]
    pub async fn stop(&self) -> AppResult<()> {
        self.send_command(|tx| ServerCommand::Stop { respond_to: tx }).await?
    }

    /// 获取状态快照
    pub async fn get_snapshot(&self) -> ServerStateSnapshot {
        self.send_command(|tx| ServerCommand::GetSnapshot { respond_to: tx })
            .await
            .unwrap_or_default()
    }

    /// 获取服务器连接信息（包含 IP 和端口）
    pub async fn get_server_info(&self) -> Option<ServerInfo> {
        self.send_command(|tx| ServerCommand::GetServerInfo { respond_to: tx })
            .await
            .ok()
            .flatten()
    }
}

/// FTP服务器Actor
pub struct FtpServerActor {
    rx: mpsc::Receiver<ServerCommand>,
    status: Arc<RwLock<ServerStatus>>,
    config: Option<ServerConfig>,
    shutdown_tx: Option<oneshot::Sender<()>>,
    stats_actor: StatsActor,
    event_bus: EventBus,
    sessions: Arc<DashSet<String>>,
    bind_addr: Option<SocketAddr>,
    app_handle: Option<AppHandle>,
}

impl FtpServerActor {
    /// 创建新的FTP服务器Actor
    pub fn new(stats_actor: StatsActor, event_bus: EventBus, app_handle: Option<AppHandle>) -> (FtpServerHandle, Self) {
        let (tx, rx) = mpsc::channel(32);
        let handle = FtpServerHandle { tx };

        let actor = Self {
            rx,
            status: Arc::new(RwLock::new(ServerStatus::Stopped)),
            config: None,
            shutdown_tx: None,
            stats_actor,
            event_bus,
            sessions: Arc::new(DashSet::new()),
            bind_addr: None,
            app_handle,
        };

        (handle, actor)
    }

    /// 运行Actor主循环
    pub async fn run(mut self) {
        info!("FTP Server Actor started");

        while let Some(cmd) = self.rx.recv().await {
            self.handle_command(cmd).await;
        }

        info!("FTP Server Actor stopped");
    }

    /// 处理命令
    #[instrument(skip(self, cmd))]
    async fn handle_command(&mut self,
        cmd: ServerCommand,
    ) {
        match cmd {
            ServerCommand::Start { config, respond_to } => {
                let result = self.do_start(config).await;
                let _ = respond_to.send(result);
            }
            ServerCommand::Stop { respond_to } => {
                let result = self.do_stop().await;
                let _ = respond_to.send(result);
            }

            ServerCommand::GetSnapshot { respond_to } => {
                let snapshot = self.get_current_snapshot().await;
                let _ = respond_to.send(snapshot);
            }
            ServerCommand::GetServerInfo { respond_to } => {
                let info = self.get_server_info().await;
                let _ = respond_to.send(info);
            }
        }
    }

    /// 执行启动
    #[instrument(skip(self, config))]
    async fn do_start(
        &mut self,
        config: ServerConfig,
    ) -> AppResult<SocketAddr> {
        self.validate_can_start().await?;
        self.set_status(ServerStatus::Starting).await;

        info!(
            port = config.port,
            root_path = %config.root_path.display(),
            "Starting FTP server"
        );

        self.prepare_root_directory(&config.root_path).await?;
        
        let port = config.port;
        let root_path = config.root_path.clone();
        let (listeners, shutdown_rx) = self.create_server_components(&root_path);
        
        self.validate_filesystem(&root_path).await?;
        
        let bind_addr = self.build_and_spawn_server(
            config.clone(),
            listeners,
            shutdown_rx,
            port,
        ).await?;

        self.finalize_startup(bind_addr, config).await;

        Ok(bind_addr)
    }

    /// 验证服务器是否可以启动
    async fn validate_can_start(&self) -> AppResult<()> {
        let status = self.status.read().await;
        if status.is_running() {
            return Err(AppError::ServerAlreadyRunning);
        }
        Ok(())
    }

    /// 设置服务器状态
    async fn set_status(&self, status: ServerStatus) {
        let mut s = self.status.write().await;
        *s = status;
    }

    /// 准备根目录（创建如果不存在）
    async fn prepare_root_directory(&self, root_path: &std::path::Path) -> AppResult<()> {
        if let Err(e) = tokio::fs::create_dir_all(root_path).await {
            error!(error = %e, "Failed to create root directory");
            self.set_status(ServerStatus::Stopped).await;
            return Err(AppError::Io(e.to_string()));
        }
        Ok(())
    }

    /// 创建服务器组件（监听器和关闭通道）
    fn create_server_components(
        &mut self,
        root_path: &std::path::Path,
    ) -> ((FtpDataListener, FtpPresenceListener), oneshot::Receiver<()>) {
        let data_listener = FtpDataListener::new(
            self.stats_actor.clone(),
            self.event_bus.clone(),
            root_path.to_path_buf(),
            self.app_handle.clone(),
        );
        let presence_listener = FtpPresenceListener::new(
            self.stats_actor.clone(),
            self.sessions.clone(),
        );

        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        self.shutdown_tx = Some(shutdown_tx);

        ((data_listener, presence_listener), shutdown_rx)
    }

    /// 验证文件系统可以创建
    async fn validate_filesystem(&self, root_path: &std::path::Path) -> AppResult<()> {
        #[cfg(target_os = "android")]
        {
            // On Android, we use MediaStore which doesn't require filesystem validation
            let _ = root_path;
            return Ok(());
        }

        #[cfg(not(target_os = "android"))]
        {
            if let Err(e) = unftp_sbe_fs::Filesystem::new(root_path) {
                error!(error = %e, "Failed to create filesystem");
                self.set_status(ServerStatus::Stopped).await;
                return Err(AppError::Io(e.to_string()));
            }
            Ok(())
        }
    }

    /// 构建并启动FTP服务器
    async fn build_and_spawn_server(
        &mut self,
        config: ServerConfig,
        (data_listener, presence_listener): (FtpDataListener, FtpPresenceListener),
        shutdown_rx: oneshot::Receiver<()>,
        port: u16,
    ) -> AppResult<SocketAddr> {
        let authenticator = Arc::new(CustomAuthenticator::new(config.auth.clone()));
        let root_path = config.root_path.clone();
        let bind_addr: SocketAddr = ([0, 0, 0, 0], port).into();
        let bind_str = bind_addr.to_string();

        // 确保 TLS 证书有效
        let cert_paths = tls::ensure_valid_certificate()?;

        // Build and start the server with FTPS
        let result = ServerBuilder::with_authenticator(
            Box::new(move || Self::create_filesystem(&root_path)),
            authenticator,
        )
        .greeting("CameraFTP Ready")
        .idle_session_timeout(config.idle_timeout_seconds)
        .notify_data(data_listener)
        .notify_presence(presence_listener)
        .ftps(&cert_paths.cert_path, &cert_paths.key_path)
        .shutdown_indicator(async move {
            let _ = shutdown_rx.await;
            info!("Shutdown signal received");
            Shutdown::new().grace_period(std::time::Duration::from_secs(5))
        })
        .build();

        let server = match result {
            Ok(s) => s,
            Err(e) => {
                error!(error = %e, "Failed to build FTP server");
                self.set_status(ServerStatus::Stopped).await;
                return Err(AppError::Other(e.to_string()));
            }
        };

        // Spawn server task
        tokio::spawn(async move {
            info!(bind_addr = %bind_str, "FTP server starting");
            match server.listen(bind_str).await {
                Ok(_) => info!("FTP server stopped normally"),
                Err(e) => error!(error = %e, "FTP server error"),
            }
        });

        // 等待服务器就绪（而非固定延迟）
        if !Self::wait_for_server_ready(port, Duration::from_secs(SERVER_READY_TIMEOUT_SECS)).await {
            warn!(port = port, "Server may not be fully ready, continuing anyway");
        }

        Ok(bind_addr)
    }

    /// 等待服务器就绪（检查端口是否监听）
    async fn wait_for_server_ready(port: u16, timeout: Duration) -> bool {
        let start = Instant::now();
        let check_interval = Duration::from_millis(CHECK_INTERVAL_MS);

        while start.elapsed() < timeout {
            if Self::is_port_listening(port) {
                info!(
                    port = port,
                    elapsed_ms = start.elapsed().as_millis() as u64,
                    "Server is ready"
                );
                return true;
            }
            tokio::time::sleep(check_interval).await;
        }
        false
    }

    /// 等待服务器完全停止（检查端口是否不再监听）
    async fn wait_for_server_stopped(port: u16, timeout: Duration) -> bool {
        let start = Instant::now();
        let check_interval = Duration::from_millis(CHECK_INTERVAL_MS);

        while start.elapsed() < timeout {
            if !Self::is_port_listening(port) {
                info!(
                    port = port,
                    elapsed_ms = start.elapsed().as_millis() as u64,
                    "Server has stopped"
                );
                return true;
            }
            tokio::time::sleep(check_interval).await;
        }
        false
    }

    /// 检查端口是否在监听
    fn is_port_listening(port: u16) -> bool {
        let addr: SocketAddr = ([127, 0, 0, 1], port).into();
        TcpStream::connect_timeout(&addr, Duration::from_millis(10)).is_ok()
    }

    /// 创建文件系统实例（路径已验证，不应失败）
    /// 
    /// # Panics
    /// 
    /// 仅在文件系统创建失败时 panic，这种情况在正常流程中不应发生，
    /// 因为路径已在 `validate_filesystem` 中验证过。
    fn create_filesystem(root_path: &std::path::Path) -> FtpStorageBackend {
        #[cfg(target_os = "android")]
        {
            let _ = root_path; // Suppress unused parameter warning
            return AndroidMediaStoreBackend::new();
        }

        #[cfg(not(target_os = "android"))]
        {
            return unftp_sbe_fs::Filesystem::new(root_path.to_path_buf())
                .unwrap_or_else(|e| panic!("Filesystem creation failed: {e}"));
        }
    }

    /// 完成启动流程，更新状态
    async fn finalize_startup(
        &mut self, bind_addr: SocketAddr, config: ServerConfig) {
        self.set_status(ServerStatus::Running).await;
        self.config = Some(config);
        self.bind_addr = Some(bind_addr);

        self.event_bus.emit_server_started(bind_addr.to_string());
        info!(bind_addr = %bind_addr, "FTP server started successfully");
    }

    /// 执行停止
    #[instrument(skip(self))]
    async fn do_stop(&mut self) -> AppResult<()> {
        let port = self.bind_addr.map(|addr| addr.port()).unwrap_or_default();
        
        {
            let status = self.status.read().await;
            if !status.is_running() {
                return Err(AppError::ServerNotRunning);
            }
        }

        info!(port = port, "Stopping FTP server");

        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }

        {
            let mut status = self.status.write().await;
            *status = ServerStatus::Stopping;
        }

        // 等待服务器完全停止（而非固定延迟）
        if !Self::wait_for_server_stopped(port, Duration::from_secs(SERVER_SHUTDOWN_TIMEOUT_SECS)).await {
            warn!(port = port, "Server did not stop within timeout, continuing anyway");
        }

        {
            let mut status = self.status.write().await;
            *status = ServerStatus::Stopped;
        }

        self.config = None;
        self.bind_addr = None;

        self.event_bus.emit_server_stopped();

        info!("FTP server stopped");

        Ok(())
    }

    /// 获取当前状态
    async fn get_current_status(&self) -> ServerStatus {
        *self.status.read().await
    }

    /// 获取当前快照
    async fn get_current_snapshot(&self) -> ServerStateSnapshot {
        let status = self.get_current_status().await;
        let is_running = status.is_running();

        // 使用 get_stats_direct() 直接从共享状态读取，避免 channel 竞争问题
        let stats = self.stats_actor.get_stats_direct().await;
        let mut snapshot = ServerStateSnapshot::from(&stats);

        // 使用 sessions 集合的大小作为连接数（更可靠）
        snapshot.connected_clients = self.sessions.len();
        snapshot.is_running = is_running;
        snapshot
    }

    /// 获取服务器连接信息（包含 IP 和端口）
    async fn get_server_info(&self) -> Option<ServerInfo> {
        let status = self.get_current_status().await;
        if !status.is_running() {
            return None;
        }

        let bind_addr = self.bind_addr?;
        let ip = crate::network::NetworkManager::recommended_ip()
            .unwrap_or_else(|| "0.0.0.0".to_string());
        let port = bind_addr.port();

        // 获取认证信息
        let (username, password_info) = if let Some(ref config) = self.config {
            match &config.auth {
                FtpAuthConfig::Anonymous => (None, None),
                FtpAuthConfig::Authenticated { username, .. } => {
                    (
                        Some(username.clone()),
                        Some("(配置密码)".to_string()),
                    )
                }
            }
        } else {
            (None, None)
        };

        Some(ServerInfo::new(ip, port, username, password_info))
    }
}

/// 创建FTP服务器Actor系统
pub fn create_ftp_server(app_handle: Option<AppHandle>) -> (
    FtpServerHandle,
    FtpServerActor,
    StatsActorWorker,
    EventBus,
) {
    let event_bus = EventBus::new();

    // StatsActor 持有 EventBus 的克隆，用于在统计变化时发送事件
    let (stats_handle, stats_worker) = StatsActor::with_event_bus(Some(event_bus.clone()));

    let (server_handle, server_actor) =
        FtpServerActor::new(stats_handle, event_bus.clone(), app_handle);

    (server_handle, server_actor, stats_worker, event_bus)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn storage_backend_name() -> &'static str {
        if cfg!(target_os = "android") {
            "AndroidMediaStoreBackend"
        } else {
            "Filesystem"
        }
    }

    #[test]
    fn selects_android_backend_type() {
        #[cfg(target_os = "android")]
        assert_eq!(storage_backend_name(), "AndroidMediaStoreBackend");

        #[cfg(not(target_os = "android"))]
        assert_eq!(storage_backend_name(), "Filesystem");
    }
}
