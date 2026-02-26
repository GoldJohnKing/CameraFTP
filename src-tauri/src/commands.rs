use tauri::{command, AppHandle, Emitter, State};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info, instrument};

use crate::config::AppConfig;
use crate::error::AppError;
use crate::ftp::types::{ServerInfo, ServerStateSnapshot};
use crate::ftp::FtpServerHandle;
use crate::network::NetworkManager;

/// FTP 服务器状态（使用 Arc<Mutex> 包装以支持异步操作）
pub struct FtpServerState(pub Arc<Mutex<Option<FtpServerHandle>>>);

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
        Default::default()
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

    Ok(ServerInfo::new(ctx.ip.clone(), ctx.port))
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

/// 设置开机自启
#[tauri::command]
pub fn set_autostart_command(enable: bool) -> Result<(), String> {
    crate::platform::get_platform().set_autostart(enable)
}

/// 获取开机自启状态
#[tauri::command]
pub fn get_autostart_status() -> Result<bool, String> {
    crate::platform::get_platform().is_autostart_enabled()
}

/// 退出应用程序
#[tauri::command]
pub fn quit_application(app: tauri::AppHandle) {
    tracing::info!("Application quit requested");
    app.exit(0);
}

/// 隐藏主窗口
#[tauri::command]
pub fn hide_main_window(app: tauri::AppHandle) -> Result<(), String> {
    tracing::info!("Hiding main window");
    crate::platform::get_platform().hide_main_window(&app)
}

/// 选择保存目录
#[tauri::command]
pub async fn select_save_directory(app: AppHandle) -> Result<Option<String>, String> {
    let platform = crate::platform::get_platform();
    let result = platform.select_save_directory(&app)?;
    
    // 如果平台返回 None（如 Windows），则使用对话框选择
    #[cfg(not(target_os = "android"))]
    if result.is_none() {
        use tauri_plugin_dialog::DialogExt;

        let folder_path = tokio::task::spawn_blocking(move || {
            app.dialog()
                .file()
                .set_title("选择存储路径")
                .blocking_pick_folder()
        })
        .await
        .map_err(|e| format!("Task failed: {}", e))?;

        return Ok(folder_path.and_then(|p| p.as_path().map(|path| path.to_string_lossy().to_string())));
    }
    
    Ok(result)
}

/// 验证保存路径是否有效
#[tauri::command]
pub fn validate_save_path(path: String) -> bool {
    let path_obj = std::path::PathBuf::from(&path);
    path_obj.exists() && path_obj.is_dir()
}

/// 获取固定存储路径（Android）或当前配置路径（桌面）
#[tauri::command]
pub fn get_storage_path() -> Result<String, String> {
    crate::platform::get_platform().get_storage_path()
}

/// 获取当前平台名称
#[tauri::command]
pub fn get_platform() -> String {
    crate::platform::get_platform().name().to_string()
}

/// 打开"所有文件访问权限"设置页面（Android）
#[tauri::command]
pub fn open_all_files_access_settings(app: tauri::AppHandle) -> Result<(), String> {
    crate::platform::get_platform().open_all_files_access_settings(&app)
}
