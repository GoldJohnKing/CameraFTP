use tauri::{command, AppHandle, Emitter, State};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info, instrument, warn};
use ts_rs::TS;

use crate::config::AppConfig;
use crate::error::AppError;
use crate::ftp::types::{ServerConfig, ServerStateSnapshot};
use crate::ftp::FtpServerHandle;
use crate::network::NetworkManager;

/// FTP 服务器状态（使用 Arc<Mutex> 包装以支持异步操作）
pub struct FtpServerState(pub Arc<Mutex<Option<FtpServerHandle>>>);

/// 服务器信息（返回给前端）
#[derive(Debug, Clone, serde::Serialize, TS)]
#[ts(export)]
pub struct ServerInfo {
    pub is_running: bool,
    pub ip: String,
    pub port: u16,
    pub url: String,
    pub username: String,
    pub password_info: String,
}

/// 统计更新（推送到前端）
#[derive(Debug, Clone, serde::Serialize)]
pub struct StatsUpdate {
    pub connected_clients: usize,
    pub files_received: u64,
    pub bytes_received: u64,
    pub last_file: Option<String>,
}

#[command]
#[instrument(skip(state, app))]
pub async fn start_server(
    state: State<'_, FtpServerState>,
    app: AppHandle,
) -> Result<ServerInfo, AppError> {
    info!("Starting FTP server...");

    let config = AppConfig::load();

    // 检查是否已在运行
    {
        let server_guard = state.0.lock().await;
        if server_guard.is_some() {
            warn!("Server already running");
            return Err(AppError::ServerAlreadyRunning);
        }
    }

    // 确保保存目录存在
    if let Err(e) = tokio::fs::create_dir_all(&config.save_path).await {
        error!(error = %e, path = %config.save_path.display(), "Failed to create save directory");
        return Err(AppError::from(e));
    }

    // 查找可用端口
    let port = if NetworkManager::is_port_available(config.port).await {
        config.port
    } else {
        warn!(requested_port = config.port, "Port not available, searching for alternative");
        NetworkManager::find_available_port(1025)
            .await
            .ok_or_else(|| {
                error!("No available port found");
                AppError::NoAvailablePort
            })?
    };

    // 获取推荐 IP
    let ip = NetworkManager::recommended_ip()
        .ok_or_else(|| {
            error!("No network interface available");
            AppError::NoNetworkInterface
        })?;

    // 创建并启动服务器
    let server_config = ServerConfig {
        port,
        root_path: config.save_path.clone(),
        allow_anonymous: true,
        passive_port_range: (50000, 50100),
        idle_timeout_seconds: 600,
    };

    // 创建FTP服务器Actor
    let (server_handle, server_actor, _stats_worker, event_bus) =
        crate::ftp::create_ftp_server();

    // 在后台运行服务器Actor
    let actor_handle = tokio::spawn(async move {
        server_actor.run().await;
    });

    // 启动服务器
    match server_handle.start(server_config).await {
        Ok(bind_addr) => {
            info!(
                bind_addr = %bind_addr,
                ip = %ip,
                port = port,
                "FTP server started successfully"
            );

            // 存储服务器句柄
            {
                let mut server_guard = state.0.lock().await;
                *server_guard = Some(server_handle.clone());
            }

            // 启动事件处理器（将领域事件转换为前端事件）
            let app_handle = app.clone();
            tokio::spawn(async move {
                let processor = crate::ftp::EventProcessor::new(&event_bus,
                ).register(crate::ftp::StatsEventHandler::new(app_handle, 500));

                processor.run().await;
            });

            // 发送兼容旧版本的事件
            let _ = app.emit("server-started", (ip.clone(), port));

            Ok(ServerInfo {
                is_running: true,
                ip: ip.clone(),
                port,
                url: format!("ftp://{}:{}", ip, port),
                username: "anonymous".to_string(),
                password_info: "(任意密码)".to_string(),
            })
        }
        Err(e) => {
            error!(error = %e, "Failed to start FTP server");

            // 清理Actor任务
            actor_handle.abort();

            Err(e.into())
        }
    }
}

#[command]
#[instrument(skip(state, app))]
pub async fn stop_server(
    state: State<'_, FtpServerState>,
    app: AppHandle,
) -> Result<(), AppError> {
    info!("Stopping FTP server...");

    let mut server_guard = state.0.lock().await;

    if let Some(server) = server_guard.take() {
        match server.stop().await {
            Ok(_) => {
                let _ = app.emit("server-stopped", ());
                info!("FTP server stopped successfully");
                Ok(())
            }
            Err(e) => {
                error!(error = %e, "Error stopping server");
                Err(e.into())
            }
        }
    } else {
        warn!("Server not running, cannot stop");
        Err(AppError::ServerNotRunning)
    }
}

#[command]
#[instrument(skip(state))]
pub async fn get_server_status(
    state: State<'_, FtpServerState>,
) -> Result<Option<ServerStateSnapshot>, AppError> {
    let server_guard = state.0.lock().await;

    if let Some(server) = server_guard.as_ref() {
        let snapshot = server.get_snapshot().await;
        Ok(Some(snapshot))
    } else {
        Ok(None)
    }
}

#[command]
#[instrument]
pub fn get_network_info() -> Result<Vec<crate::network::NetworkInterface>, AppError> {
    Ok(NetworkManager::list_interfaces())
}

#[command]
#[instrument]
pub fn load_config() -> AppConfig {
    AppConfig::load()
}

#[command]
#[instrument(skip(config))]
pub fn save_config(config: AppConfig) -> Result<(), AppError> {
    config.save()?;
    info!("Configuration saved successfully");
    Ok(())
}

#[command]
#[instrument]
pub async fn check_port_available(port: u16) -> bool {
    NetworkManager::is_port_available(port).await
}

#[command]
#[instrument(skip(state))]
pub async fn get_diagnostic_info(
    state: State<'_, FtpServerState>,
) -> Result<Option<crate::ftp::types::DiagnosticInfo>, AppError> {
    let server_guard = state.0.lock().await;

    if let Some(server) = server_guard.as_ref() {
        let info = server.get_diagnostic_info().await;
        Ok(Some(info))
    } else {
        Ok(None)
    }
}
