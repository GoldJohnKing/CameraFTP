use tauri::{command, AppHandle, Emitter, State};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info};

use crate::config::AppConfig;
use crate::ftp::{FtpServer, ServerConfig, ServerStateSnapshot};
use crate::network::NetworkManager;

/// FTP 服务器状态（使用 Arc<Mutex> 包装以支持异步操作）
pub struct FtpServerState(pub Arc<Mutex<Option<FtpServer>>>);

/// 服务器信息（返回给前端）
#[derive(Debug, Clone, serde::Serialize)]
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
pub async fn start_server(
    state: State<'_, FtpServerState>,
    app: AppHandle,
) -> Result<ServerInfo, String> {
    info!("Starting FTP server...");

    let config = AppConfig::load();

    // 检查是否已在运行
    {
        let server_guard = state.0.lock().await;
        if server_guard.is_some() {
            return Err("Server is already running".to_string());
        }
    }

    // 确保保存目录存在
    if let Err(e) = tokio::fs::create_dir_all(&config.save_path).await {
        return Err(format!("Failed to create save directory: {}", e));
    }

    // 查找可用端口
    let port = if NetworkManager::is_port_available(config.port).await {
        config.port
    } else {
        NetworkManager::find_available_port(1025)
            .await
            .ok_or("No available port found")?
    };

    // 获取推荐 IP
    let ip = NetworkManager::recommended_ip().ok_or("No network interface available")?;

    // 创建并启动服务器
    let server_config = ServerConfig {
        port,
        root_path: config.save_path.clone(),
        allow_anonymous: true,
    };

    let mut server = FtpServer::new(server_config);
    match server.start().await {
        Ok(addr) => {
            info!("FTP server started on {}:{}", ip, port);

            // 存储服务器实例
            {
                let mut server_guard = state.0.lock().await;
                *server_guard = Some(server);
            }

            // 发送事件
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
            error!("Failed to start server: {}", e);
            Err(format!("Failed to start server: {}", e))
        }
    }
}

#[command]
pub async fn stop_server(state: State<'_, FtpServerState>, app: AppHandle) -> Result<(), String> {
    info!("Stopping FTP server...");

    let mut server_guard = state.0.lock().await;

    if let Some(mut server) = server_guard.take() {
        server.stop();
        let _ = app.emit("server-stopped", ());
        info!("FTP server stopped");
        Ok(())
    } else {
        Err("Server is not running".to_string())
    }
}

#[command]
pub async fn get_server_status(
    state: State<'_, FtpServerState>,
) -> Result<Option<ServerStateSnapshot>, String> {
    let server_guard = state.0.lock().await;

    if let Some(server) = server_guard.as_ref() {
        Ok(Some(server.state_snapshot()))
    } else {
        Ok(None)
    }
}

#[command]
pub fn get_network_info() -> Result<Vec<crate::network::NetworkInterface>, String> {
    Ok(NetworkManager::list_interfaces())
}

#[command]
pub fn load_config() -> AppConfig {
    AppConfig::load()
}

#[command]
pub fn save_config(config: AppConfig) -> Result<(), String> {
    config.save().map_err(|e| e.to_string())
}

#[command]
pub async fn check_port_available(port: u16) -> bool {
    NetworkManager::is_port_available(port).await
}
