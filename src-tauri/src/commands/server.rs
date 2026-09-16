// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use tauri::{command, AppHandle, State};
use tracing::{error, info, instrument};

use crate::commands::FtpServerState;
use crate::error::AppError;
use crate::ftp::types::{FtpServerSlot, ServerInfo, ServerRuntimeView, ServerStateSnapshot};
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
    // 注：Starting 窗口（另一任务启动中）无运行时信息可返回，不在此时提前返回，
    // 而是落入工厂的认领检查（start_ftp_server），按 ServerAlreadyRunning 拒绝
    {
        let server_guard = state.0.lock().await;
        if let Some(server) = server_guard.running_handle() {
            if let Some(info) = server.get_server_info().await {
                info!(ip = %info.ip, port = info.port, "Server already running, returning current state");
                return Ok(info);
            }
        }
    }

    let ctx = crate::ftp::server_factory::start_server_with_event_pipeline(
        &state.0,
        app.clone(),
        Duration::from_secs(2),
    ).await?;

    info!(
        ip = %ctx.ip,
        port = ctx.port,
        "FTP server started successfully"
    );

    let (username, password_info) = ctx.display_credentials;

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
        server_guard.running_handle().cloned()
    };

    if let Some(server) = server {
        match server.stop().await {
            Ok(_) => {
                let mut server_guard = state.0.lock().await;
                *server_guard = FtpServerSlot::None;

                info!("FTP server stopped successfully");
                Ok(())
            }
            Err(e) => {
                let runtime_state = server.runtime_state().current_snapshot().await;

                if !runtime_state.is_running {
                    // 补偿逻辑保留：句柄可能因 Actor 任务异常退出而“陈旧”
                    // （sentinel 无法覆盖 Actor 自身崩溃的场景，仍需在此清理）
                    let mut server_guard = state.0.lock().await;
                    *server_guard = FtpServerSlot::None;

                    info!(error = %e, "Stop returned an error after the server had already stopped; cleared stale server handle");
                    Ok(())
                } else {
                    error!(error = %e, "Error stopping server");
                    Err(e)
                }
            }
        }
    } else {
        // 幂等性：服务器未运行时静默返回成功。
        // 注：Starting 窗口内（另一任务正在启动中）同样取不到句柄，
        // 此时 stop 请求被忽略并返回成功 —— 与既有行为一致
        //（启动完成后的下一次 stop 才会真正停止服务器）
        info!("Server not running, returning success (idempotent)");
        Ok(())
    }
}

#[command]
#[instrument(skip(state))]
pub async fn get_server_runtime_state(
    state: State<'_, FtpServerState>,
) -> Result<ServerRuntimeView, AppError> {
    // Starting 窗口内取不到句柄，返回默认（未运行）视图 —— 与旧行为一致
    // （旧代码在启动窗口内字段同样为 None）
    let server_handle = {
        let server_guard = state.0.lock().await;
        server_guard.running_handle().cloned()
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
