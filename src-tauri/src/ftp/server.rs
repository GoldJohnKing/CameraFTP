use crate::error::{AppError, AppResult};
use crate::ftp::events::EventBus;
use crate::ftp::listeners::{FtpDataListener, FtpPresenceListener};
use crate::ftp::stats::{StatsActor, StatsActorWorker};
use crate::ftp::types::{
    ServerConfig, ServerInfo, ServerStateSnapshot, ServerStatus, FtpAuthConfig,
};
use dashmap::DashSet;
use libunftp::options::Shutdown;
use libunftp::ServerBuilder;
use std::net::SocketAddr;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::{mpsc, oneshot, RwLock};
use tracing::{error, info, instrument};
use unftp_core::auth::{Authenticator, Credentials, AuthenticationError, Principal};

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

/// FTP服务器Actor命令
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

/// FTP服务器Actor句柄
#[derive(Debug, Clone)]
pub struct FtpServerHandle {
    tx: mpsc::Sender<ServerCommand>,
}

impl FtpServerHandle {
    /// Helper method to send a command and receive the response
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
        self.send_command(|tx| ServerCommand::Start { config, respond_to: tx })
            .await
            .map_err(|_| AppError::ServerNotRunning)?
    }

    /// 停止服务器
    #[instrument(skip(self))]
    pub async fn stop(&self) -> AppResult<()> {
        self.send_command(|tx| ServerCommand::Stop { respond_to: tx })
            .await
            .map_err(|_| AppError::ServerNotRunning)?
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
        // 检查状态
        {
            let status = self.status.read().await;
            if status.is_running() {
                return Err(AppError::ServerAlreadyRunning);
            }
        }

        // 更新状态为启动中
        {
            let mut status = self.status.write().await;
            *status = ServerStatus::Starting;
        }

        info!(
            port = config.port,
            root_path = %config.root_path.display(),
            "Starting FTP server"
        );

        // 确保目录存在
        if let Err(e) = tokio::fs::create_dir_all(&config.root_path).await {
            error!(error = %e, "Failed to create root directory");
            {
                let mut status = self.status.write().await;
                *status = ServerStatus::Stopped;
            }
            return Err(AppError::Io(e.to_string()));
        }

        // 创建监听器
        let root_path = config.root_path.clone();
        let data_listener = FtpDataListener::new(self.stats_actor.clone(), self.event_bus.clone(), root_path.clone(), self.app_handle.clone());
        let presence_listener =
            FtpPresenceListener::new(self.stats_actor.clone(), self.sessions.clone());

        // 创建关闭通道
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        self.shutdown_tx = Some(shutdown_tx);
        let port = config.port;

        // 预先验证文件系统创建，避免运行时闭包内panic
        // Filesystem 不实现 Clone，Arc<Filesystem> 不实现 StorageBackend
        // 因此只能在闭包内创建新实例，但先验证路径有效性
        if let Err(e) = unftp_sbe_fs::Filesystem::new(root_path.clone()) {
            error!(error = %e, "Failed to create filesystem");
            {
                let mut status = self.status.write().await;
                *status = ServerStatus::Stopped;
            }
            return Err(AppError::Io(e.to_string()));
        }

        // 创建认证器
        let authenticator = Arc::new(CustomAuthenticator::new(config.auth.clone()));

        // 构建并启动服务器
        // SAFETY: 闭包内创建 Filesystem 实例。路径已在上方（line 257）验证有效。
        // Filesystem 不实现 Clone，Arc<Filesystem> 不实现 StorageBackend，
        // 因此只能在闭包内创建新实例。
        // 
        // 注意：如果创建失败（理论上不可能，因为路径已验证），会记录严重错误日志。
        // 这是设计上的限制：ServerBuilder 要求闭包返回 Filesystem 而非 Result，
        // 因此无法优雅地传播错误。路径预验证确保这种情况极不可能发生。
        // 注意：PASV 端口使用 libunftp 默认范围 49152-65535
        let result = ServerBuilder::with_authenticator(
            Box::new(move || {
                unftp_sbe_fs::Filesystem::new(root_path.clone())
                    .map_err(|e| {
                        // 路径已验证，这里不应该失败
                        // 但如果确实失败，记录严重错误并返回一个会失败的 filesystem
                        error!(
                            error = %e,
                            root_path = %root_path.display(),
                            "CRITICAL: Filesystem creation failed in ServerBuilder closure despite validation"
                        );
                        e
                    })
                    // 使用 expect 因为这是一个不应发生的内部错误
                    // 如果到达这里，说明预验证逻辑有 bug
                    .expect("Filesystem creation failed after successful validation")
            }),
            authenticator,
        )
        .greeting("Camera FTP Companion Ready")
        .idle_session_timeout(config.idle_timeout_seconds)
        .notify_data(data_listener)
        .notify_presence(presence_listener)
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
                {
                    let mut status = self.status.write().await;
                    *status = ServerStatus::Stopped;
                }
                return Err(AppError::Other(e.to_string()));
            }
        };

        let bind_addr: SocketAddr = ([0, 0, 0, 0], port).into();
        let bind_str = bind_addr.to_string();

        // 启动服务器任务
        tokio::spawn(async move {
            info!(bind_addr = %bind_str, "FTP server starting");

            match server.listen(bind_str.clone()).await {
                Ok(_) => {
                    info!("FTP server stopped normally");
                }
                Err(e) => {
                    error!(error = %e, "FTP server error");
                }
            }
        });

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        {
            let mut s = self.status.write().await;
            *s = ServerStatus::Running;
        }
        self.config = Some(config);
        self.bind_addr = Some(bind_addr);

        self.event_bus.emit_server_started(bind_addr.to_string());

        info!(bind_addr = %bind_addr, "FTP server started successfully");

        Ok(bind_addr)
    }

    /// 执行停止
    #[instrument(skip(self))]
    async fn do_stop(&mut self) -> AppResult<()> {
        {
            let status = self.status.read().await;
            if !status.is_running() {
                return Err(AppError::ServerNotRunning);
            }
        }

        info!("Stopping FTP server");

        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }

        {
            let mut status = self.status.write().await;
            *status = ServerStatus::Stopping;
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

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
