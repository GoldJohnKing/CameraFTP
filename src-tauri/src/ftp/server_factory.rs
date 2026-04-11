// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! 服务器工厂 - 统一服务器启动逻辑

use crate::config_service::ConfigService;
use crate::constants::{
    DEFAULT_FTP_PORT_WINDOWS, DEFAULT_FTP_PORT_ANDROID,
    MIN_PORT, IDLE_TIMEOUT_SECONDS,
};
use crate::error::AppError;
use crate::ftp::{
    create_ftp_server, EventBus, EventProcessor, FtpServerHandle, FtpAuthConfig,
    FrontendTransientEventHandler, ServerConfig, StatsEventHandler, TrayUpdateHandler,
};
use crate::network::NetworkManager;
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tokio::sync::{Mutex, oneshot};
use tracing::{error, info, warn};

#[derive(Debug)]
pub struct ServerStartupContext {
    pub port: u16,
    pub ip: String,
    pub event_bus: EventBus,
}

#[derive(Debug, Clone)]
pub struct ServerStartupOptions {
    pub min_port: u16,
}

impl Default for ServerStartupOptions {
    fn default() -> Self {
        Self {
            min_port: MIN_PORT,
        }
    }
}

pub async fn start_ftp_server(
    state: &Arc<Mutex<Option<FtpServerHandle>>>,
    options: ServerStartupOptions,
    app_handle: AppHandle,
) -> Result<ServerStartupContext, AppError> {
    // 检查是否已在运行
    {
        let guard = state.lock().await;
        if guard.is_some() {
            return Err(AppError::ServerAlreadyRunning);
        }
    }

    let config_service = app_handle.state::<Arc<ConfigService>>();
    let config = config_service
        .get()
        .map_err(|e| AppError::Other(format!("Failed to read config from service: {}", e)))?;

    // 统一通过 PlatformService 验证存储路径
    // 这会处理平台特定的权限检查和目录创建
    let save_path = crate::platform::get_platform()
        .ensure_storage_ready(&app_handle)
        .map_err(|e| {
            error!(error = %e, "Storage not ready");
            AppError::StoragePermissionError(e)
        })?;

    // 更新配置中的保存路径（可能与验证后的路径不同）
    let save_path = std::path::PathBuf::from(save_path);

    // 查找可用端口
    // 当 advanced_connection 禁用时，Windows 使用默认端口 21，Android 使用 2121
    let default_port = if cfg!(target_os = "windows") {
        DEFAULT_FTP_PORT_WINDOWS
    } else {
        DEFAULT_FTP_PORT_ANDROID
    };
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

    // 创建服务器配置
    // 注意：PASV 端口使用 libunftp 默认范围 49152-65535（无需手动配置）
    let server_config = ServerConfig {
        port,
        root_path: save_path.clone(),
        idle_timeout_seconds: IDLE_TIMEOUT_SECONDS,
        auth: if config.advanced_connection.enabled {
            FtpAuthConfig::from(&config.advanced_connection.auth)
        } else {
            FtpAuthConfig::default()
        },
    };

    // 创建FTP服务器Actor
    let (server_handle, server_actor, stats_worker, event_bus) = create_ftp_server(Some(app_handle));

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

pub fn spawn_event_processor(app_handle: AppHandle, event_bus: &EventBus) -> oneshot::Receiver<()> {
    let app_handle_for_tray = app_handle.clone();
    let (ready_tx, ready_rx) = oneshot::channel();

    // 从 EventBus 借用组件，不获取所有权
    // 这样 event_bus 继续由调用者拥有，服务器和处理器共享同一个状态通道
    let transient_rx = event_bus.subscribe();
    let runtime_state = event_bus.runtime_state();
    let state_rx = runtime_state.subscribe();
    // 保留 runtime_state 用于直接查询，避免 watch channel 的竞态条件
    let runtime_state_for_processor = runtime_state.clone();

    tokio::spawn(async move {
        let processor = EventProcessor::from_parts(
            transient_rx,
            state_rx,
            Some(runtime_state_for_processor)
        )
            .register_runtime_state_handler(StatsEventHandler::new(app_handle.clone()))
            .register_runtime_state_handler(TrayUpdateHandler::new(app_handle_for_tray))
            .register(FrontendTransientEventHandler::new(app_handle));

        // 先运行处理器并等待其完成初始化（读取当前状态并分发给 handlers）
        // 这确保在返回 ready 信号前，所有 runtime state handlers 都已处理当前状态
        processor.run_with_ready_signal(ready_tx).await;
    });

    ready_rx
}

#[cfg(test)]
mod tests {
    #[test]
    fn startup_context_source_does_not_store_server_handle() {
        let source = include_str!("server_factory.rs");
        let forbidden = ["pub server_handle", "FtpServerHandle"].join(": ");

        assert!(!source.contains(&forbidden));
    }
}
