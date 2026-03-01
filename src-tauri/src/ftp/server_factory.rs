//! 服务器工厂 - 统一服务器启动逻辑

use crate::config::AppConfig;
use crate::error::AppError;
use crate::ftp::{
    create_ftp_server, EventBus, EventProcessor, FtpServerHandle, FtpAuthConfig, ServerConfig, StatsEventHandler,
};
use crate::network::NetworkManager;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

#[derive(Debug)]
pub struct ServerStartupContext {
    pub port: u16,
    pub ip: String,
    pub server_handle: FtpServerHandle,
    pub event_bus: EventBus,
}

#[derive(Debug, Clone)]
pub struct ServerStartupOptions {
    pub min_port: u16,
}

impl Default for ServerStartupOptions {
    fn default() -> Self {
        Self {
            min_port: 1025,
        }
    }
}

pub async fn start_ftp_server(
    state: &Arc<Mutex<Option<FtpServerHandle>>>,
    options: ServerStartupOptions,
    app_handle: Option<AppHandle>,
) -> Result<ServerStartupContext, AppError> {
    // 检查是否已在运行
    {
        let guard = state.lock().await;
        if guard.is_some() {
            return Err(AppError::ServerAlreadyRunning);
        }
    }

    let config = AppConfig::load();

    // 统一通过 PlatformService 验证存储路径
    // 这会处理平台特定的权限检查和目录创建
    let save_path = crate::platform::get_platform()
        .ensure_storage_ready()
        .map_err(|e| {
            error!(error = %e, "Storage not ready");
            AppError::StoragePermissionError(e)
        })?;

    // 更新配置中的保存路径（可能与验证后的路径不同）
    let save_path = std::path::PathBuf::from(save_path);

    // 查找可用端口
    // 当 advanced_connection 禁用时，Windows 使用默认端口 21，Android 使用 2121
    let default_port = if cfg!(target_os = "windows") { 21 } else { 2121 };
    let requested_port = if config.advanced_connection.enabled {
        config.port
    } else {
        default_port
    };

    let port = if NetworkManager::is_port_available(requested_port).await {
        requested_port
    } else if config.auto_select_port {
        warn!(
            requested_port = requested_port,
            "Port not available, searching for alternative"
        );
        NetworkManager::find_available_port(options.min_port)
            .await
            .ok_or_else(|| {
                error!("No available port found");
                AppError::NoAvailablePort
            })?
    } else {
        return Err(AppError::NoAvailablePort);
    };

    // 获取推荐IP
    let ip = NetworkManager::recommended_ip().ok_or_else(|| {
        error!("No network interface available");
        AppError::NoNetworkInterface
    })?;

    // 确定PASV端口范围
    // 简单模式: 使用默认范围 50000-50100，如果全部占用则自动查找
    // 高级模式: 使用用户配置范围，如果全部占用则拒绝启动
    let default_pasv_range = (50000, 50100);
    
    let pasv_range = if config.advanced_connection.enabled {
        // 高级模式: 使用用户配置的范围
        let user_range = (
            config.advanced_connection.pasv.port_start,
            config.advanced_connection.pasv.port_end,
        );
        
        // 验证用户配置的范围是否有可用端口
        let (available_count, total_count, _) = NetworkManager::check_pasv_port_range(
            user_range.0, user_range.1
        ).await;
        
        if available_count == 0 {
            error!(
                start = user_range.0,
                end = user_range.1,
                "All PASV ports in user-configured range are occupied"
            );
            return Err(AppError::NoAvailablePasvPort(format!(
                "{}-{} (共{}个端口均被占用)",
                user_range.0, user_range.1, total_count
            )));
        }
        
        info!(
            start = user_range.0,
            end = user_range.1,
            available = available_count,
            total = total_count,
            "Using user-configured PASV range"
        );
        
        user_range
    } else {
        // 简单模式: 先尝试默认范围
        let (available_count, total_count, _) = NetworkManager::check_pasv_port_range(
            default_pasv_range.0, default_pasv_range.1
        ).await;
        
        if available_count > 0 {
            info!(
                start = default_pasv_range.0,
                end = default_pasv_range.1,
                available = available_count,
                total = total_count,
                "Using default PASV range"
            );
            default_pasv_range
        } else {
            // 默认范围全部占用，自动查找可用范围
            warn!(
                start = default_pasv_range.0,
                end = default_pasv_range.1,
                "Default PASV range fully occupied, searching for alternative"
            );
            
            match NetworkManager::find_available_pasv_range(1024).await {
                Some((start, end)) => {
                    info!(
                        start = start,
                        end = end,
                        "Found available PASV range"
                    );
                    (start, end)
                }
                None => {
                    error!("No available PASV port range found");
                    return Err(AppError::NoAvailablePasvPort(
                        "无法找到可用的PASV端口范围，请检查系统端口占用情况".to_string()
                    ));
                }
            }
        }
    };

    // 创建服务器配置
    let server_config = ServerConfig {
        port,
        root_path: save_path.clone(),
        passive_port_range: pasv_range,
        idle_timeout_seconds: 600,
        auth: if config.advanced_connection.enabled {
            FtpAuthConfig::from(&config.advanced_connection.auth)
        } else {
            FtpAuthConfig::default()
        },
    };

    // 创建FTP服务器Actor
    let (server_handle, server_actor, stats_worker, event_bus) = create_ftp_server(app_handle);

    // 运行统计Actor Worker（必须在后台运行，否则统计不会更新）
    tokio::spawn(async move {
        stats_worker.run().await;
    });

    // 运行服务器Actor
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
                let mut guard = state.lock().await;
                *guard = Some(server_handle.clone());
            }

            Ok(ServerStartupContext {
                port,
                ip,
                server_handle,
                event_bus,
            })
        }
        Err(e) => {
            error!(error = %e, "Failed to start FTP server");
            actor_handle.abort();
            Err(e.into())
        }
    }
}

pub fn spawn_event_processor(app_handle: AppHandle, event_bus: EventBus) {
    tokio::spawn(async move {
        let processor = EventProcessor::new(&event_bus)
            .register(StatsEventHandler::new(app_handle));
        processor.run().await;
    });
}
