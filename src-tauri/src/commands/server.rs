use tauri::{command, AppHandle, State};
use tracing::{error, info, instrument};

use crate::commands::FtpServerState;
use crate::config::AppConfig;
use crate::error::AppError;
use crate::ftp::types::{ServerInfo, ServerStateSnapshot};
use crate::network::NetworkManager;

#[command]
#[instrument(skip(state, app))]
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
        Some(app.clone())
    ).await?;

    // 启动事件处理器（EventBus 会发送 server-started 事件）
    crate::ftp::server_factory::spawn_event_processor(
        app.clone(),
        ctx.event_bus,
    );

    info!(
        ip = %ctx.ip,
        port = ctx.port,
        "FTP server started successfully"
    );

    // Note: server-started event is emitted via EventBus by StatsEventHandler
    // This ensures consistent event handling through the event processor

    // 使用 PlatformService trait 更新平台状态
    crate::platform::get_platform().on_server_started(&app);

    // 加载配置获取认证信息
    let app_config = AppConfig::load();
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
                // Note: server-stopped event is emitted via EventBus by StatsEventHandler
                // (server.rs:do_stop() calls event_bus.emit_server_stopped())
                
                // 使用 PlatformService trait 更新平台状态
                crate::platform::get_platform().on_server_stopped(&app);
                
                info!("FTP server stopped successfully");
                Ok(())
            }
            Err(e) => {
                error!(error = %e, "Error stopping server");
                Err(e.into())
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
#[instrument]
pub async fn check_port_available(port: u16) -> bool {
    NetworkManager::is_port_available(port).await
}

/// 显示并置顶主窗口（桌面平台特有）
#[command]
pub fn show_main_window(app: AppHandle) -> Result<(), String> {
    crate::platform::get_platform().show_main_window(&app)
}

/// 隐藏主窗口
#[command]
pub fn hide_main_window(app: AppHandle) -> Result<(), String> {
    tracing::info!("Hiding main window");
    crate::platform::get_platform().hide_main_window(&app)
}

/// 退出应用程序
#[command]
pub fn quit_application(app: tauri::AppHandle) {
    tracing::info!("Application quit requested");
    app.exit(0);
}