// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use tauri::{command, AppHandle, Manager, State};
use tracing::{error, info, instrument};

use crate::commands::FtpServerState;
use crate::config_service::ConfigService;
use crate::error::AppError;
use crate::file_index::FileIndexService;
use crate::ftp::types::{ServerInfo, ServerRuntimeView, ServerStateSnapshot};
use std::sync::Arc;
use std::time::Duration;
use crate::network::NetworkManager;

#[command]
#[instrument(skip(state))]
pub async fn start_server(
    state: State<'_, FtpServerState>,
    app: AppHandle,
) -> Result<ServerInfo, AppError> {
    info!("Starting FTP server...");

    // 幂等性检查：如果服务器已运行，静默返回当前状态
    {
        let server_guard = state.0.lock().await;
        if let Some(server) = server_guard.as_ref() {
            if let Some(info) = server.get_server_info().await {
                info!(ip = %info.ip, port = info.port, "Server already running, returning current state");
                return Ok(info);
            }
        }
    }

    // 使用 server_factory 启动服务器
    let ctx = crate::ftp::server_factory::start_ftp_server(
        &state.0,
        Default::default(),
        app.clone()
    ).await?;

    // 先启动事件处理器，并等待其订阅建立
    let ready_rx = crate::ftp::server_factory::spawn_event_processor(
        app.clone(),
        ctx.event_bus.clone(),
    );

    if tokio::time::timeout(Duration::from_secs(2), ready_rx).await.is_err() {
        info!("Event processor readiness timed out during manual start");
    }

    // 再将事件总线交给文件索引服务，避免其在订阅建立前发射瞬时事件
    let file_index = app.state::<Arc<FileIndexService>>();
    file_index.set_event_bus(ctx.event_bus).await;

    info!(
        ip = %ctx.ip,
        port = ctx.port,
        "FTP server started successfully"
    );

    // 加载配置获取认证信息
    let config_service = app.state::<Arc<ConfigService>>();
    let app_config = config_service
        .get()
        .map_err(|e| AppError::Other(format!("Failed to read config from service: {}", e)))?;
    let (username, password_info) = if app_config.advanced_connection.enabled {
        if app_config.advanced_connection.auth.anonymous {
            (None, None)
        } else {
            (
                Some(app_config.advanced_connection.auth.username),
                Some("(配置密码)".to_string()),
            )
        }
    } else {
        (None, None)
    };

    Ok(ServerInfo::new(ctx.ip.clone(), ctx.port, username, password_info))
}

#[command]
#[instrument(skip(state))]
pub async fn stop_server(
    state: State<'_, FtpServerState>,
    app: AppHandle,
) -> Result<(), AppError> {
    info!("Stopping FTP server...");

    let server = {
        let server_guard = state.0.lock().await;
        server_guard.as_ref().cloned()
    };

    if let Some(server) = server {
        match server.stop().await {
            Ok(_) => {
                let mut server_guard = state.0.lock().await;
                server_guard.take();

                info!("FTP server stopped successfully");
                Ok(())
            }
            Err(e) => {
                let runtime_state = server.runtime_state().current_snapshot().await;

                if !runtime_state.is_running {
                    let mut server_guard = state.0.lock().await;
                    server_guard.take();

                    info!(error = %e, "Stop returned an error after the server had already stopped; cleared stale server handle");
                    Ok(())
                } else {
                    error!(error = %e, "Error stopping server");
                    Err(e.into())
                }
            }
        }
    } else {
        // 幂等性：服务器未运行时静默返回成功
        info!("Server not running, returning success (idempotent)");
        Ok(())
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
#[instrument(skip(state))]
pub async fn get_server_info(
    state: State<'_, FtpServerState>,
) -> Result<Option<ServerInfo>, AppError> {
    let server_guard = state.0.lock().await;
    if let Some(server) = server_guard.as_ref() {
        let info = server.get_server_info().await;
        Ok(info)
    } else {
        Ok(None)
    }
}

#[command]
#[instrument(skip(state))]
pub async fn get_server_runtime_state(
    state: State<'_, FtpServerState>,
) -> Result<ServerRuntimeView, AppError> {
    let server_handle = {
        let server_guard = state.0.lock().await;
        server_guard.as_ref().cloned()
    };

    let Some(server) = server_handle else {
        return Ok(ServerRuntimeView {
            server_info: None,
            stats: ServerStateSnapshot::default(),
        });
    };

    let runtime_state = server.runtime_state().current_snapshot().await;
    let server_info = if runtime_state.is_running {
        server.get_server_info().await
    } else {
        None
    };

    Ok(ServerRuntimeView {
        server_info,
        stats: runtime_state,
    })
}

#[command]
#[instrument]
pub async fn check_port_available(port: u16) -> bool {
    NetworkManager::is_port_available(port).await
}

/// 显示并置顶主窗口（桌面平台特有）
#[command]
pub fn show_main_window(app: AppHandle) -> Result<(), String> {
    platform().show_main_window(&app)
}

/// 隐藏主窗口
#[command]
pub fn hide_main_window(app: AppHandle) -> Result<(), String> {
    tracing::info!("Hiding main window");
    platform().hide_main_window(&app)
}

/// 获取平台引用（减少重复调用）
#[inline]
fn platform() -> &'static dyn crate::platform::PlatformService {
    crate::platform::get_platform()
}

/// 退出应用程序
#[command]
pub fn quit_application(app: tauri::AppHandle) {
    tracing::info!("Application quit requested");
    app.exit(0);
}
