//! 服务器工厂 - 统一服务器启动逻辑

use crate::config::AppConfig;
use crate::error::AppError;
use crate::ftp::{
    create_ftp_server, EventBus, EventProcessor, FtpServerHandle, ServerConfig, StatsEventHandler,
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
    pub actor_handle: tokio::task::JoinHandle<()>,
}

#[derive(Debug, Clone)]
pub struct ServerStartupOptions {
    pub auto_select_port: bool,
    pub min_port: u16,
}

impl Default for ServerStartupOptions {
    fn default() -> Self {
        Self {
            auto_select_port: true,
            min_port: 1025,
        }
    }
}

pub async fn start_ftp_server(
    state: &Arc<Mutex<Option<FtpServerHandle>>>,
    options: ServerStartupOptions,
) -> Result<ServerStartupContext, AppError> {
    // 检查是否已在运行
    {
        let guard = state.lock().await;
        if guard.is_some() {
            return Err(AppError::ServerAlreadyRunning);
        }
    }

    let config = AppConfig::load();

    // 确保保存目录存在
    tokio::fs::create_dir_all(&config.save_path).await.map_err(|e| {
        error!(error = %e, path = %config.save_path.display(), "Failed to create save directory");
        AppError::Other(format!(
            "无法创建保存目录 '{}': {}。请检查存储权限或更改保存路径。",
            config.save_path.display(),
            e
        ))
    })?;

    // 检查目录是否可写
    let test_file = config.save_path.join(".write_test");
    match tokio::fs::write(&test_file, b"test").await {
        Ok(_) => {
            let _ = tokio::fs::remove_file(&test_file).await;
        }
        Err(e) => {
            error!(error = %e, path = %config.save_path.display(), "Save directory is not writable");
            return Err(AppError::Other(format!(
                "保存目录 '{}' 没有写入权限 ({})。Android 用户请使用应用私有目录，或在系统设置中开启'所有文件访问权限'。",
                config.save_path.display(),
                e
            )));
        }
    }

    // 查找可用端口
    let port = if NetworkManager::is_port_available(config.port).await {
        config.port
    } else if options.auto_select_port {
        warn!(
            requested_port = config.port,
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

    // 创建服务器配置
    let server_config = ServerConfig {
        port,
        root_path: config.save_path.clone(),
        allow_anonymous: true,
        passive_port_range: (50000, 50100),
        idle_timeout_seconds: 600,
    };

    // 创建FTP服务器Actor
    let (server_handle, server_actor, stats_worker, event_bus) = create_ftp_server();

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
                actor_handle,
            })
        }
        Err(e) => {
            error!(error = %e, "Failed to start FTP server");
            actor_handle.abort();
            Err(e.into())
        }
    }
}

pub fn spawn_event_processor(app_handle: AppHandle, event_bus: EventBus, debounce_ms: u64) {
    tokio::spawn(async move {
        let processor = EventProcessor::new(&event_bus)
            .register(StatsEventHandler::new(app_handle, debounce_ms));
        processor.run().await;
    });
}
