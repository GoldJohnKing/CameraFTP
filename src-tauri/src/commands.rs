use tauri::{command, AppHandle, Emitter, Manager, State};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info, instrument, warn};
use ts_rs::TS;

use crate::config::AppConfig;
use crate::error::AppError;
use crate::ftp::types::ServerStateSnapshot;
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

#[command]
#[instrument(skip(state, app))]
pub async fn start_server(
    state: State<'_, FtpServerState>,
    app: AppHandle,
) -> Result<ServerInfo, AppError> {
    info!("Starting FTP server...");

    // 使用 server_factory 启动服务器
    let ctx = crate::ftp::server_factory::start_ftp_server(
        &state.0,
        Default::default()
    ).await?;

    // 启动事件处理器
    crate::ftp::server_factory::spawn_event_processor(
        app.clone(),
        ctx.event_bus,
        500
    );

    // 发送启动事件
    crate::ftp::server_factory::emit_server_started(&app, &ctx.ip, ctx.port);

    info!(
        ip = %ctx.ip,
        port = ctx.port,
        "FTP server started successfully"
    );

    // 更新托盘图标为 idle 状态（服务器运行中，但还没有设备连接）
    #[cfg(target_os = "windows")]
    {
        if let Err(e) = crate::platform::windows::update_tray_icon(&app, crate::platform::windows::TrayIconState::Idle) {
            warn!(error = %e, "Failed to update tray icon to idle");
        }
    }

    Ok(ServerInfo {
        is_running: true,
        ip: ctx.ip.clone(),
        port: ctx.port,
        url: format!("ftp://{}:{}", ctx.ip, ctx.port),
        username: "anonymous".to_string(),
        password_info: "(任意密码)".to_string(),
    })
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
                
                // 更新托盘图标为 stopped 状态（服务器停止）
                #[cfg(target_os = "windows")]
                {
                    if let Err(e) = crate::platform::windows::update_tray_icon(&app, crate::platform::windows::TrayIconState::Stopped) {
                        warn!(error = %e, "Failed to update tray icon to stopped");
                    }
                }
                
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

/// 设置开机自启
#[tauri::command]
pub fn set_autostart_command(_enable: bool) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        crate::platform::windows::set_autostart(_enable)
            .map_err(|e| format!("Failed to set autostart: {}", e))
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("Autostart is only supported on Windows".to_string())
    }
}

/// 获取开机自启状态
#[tauri::command]
pub fn get_autostart_status() -> Result<bool, String> {
    #[cfg(target_os = "windows")]
    {
        crate::platform::windows::is_autostart_enabled()
            .map_err(|e| format!("Failed to get autostart status: {}", e))
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(false)
    }
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
    if let Some(_window) = app.get_webview_window("main") {
        #[cfg(target_os = "windows")]
        {
            _window.hide().map_err(|e| format!("Failed to hide window: {}", e))?;
            tracing::info!("Main window hidden successfully");
        }
        #[cfg(not(target_os = "windows"))]
        {
            // 移动端不支持 hide，使用最小化或后台运行
            tracing::info!("Hide not supported on mobile, window stays visible");
        }
        Ok(())
    } else {
        Err("Main window not found".to_string())
    }
}

/// 选择保存目录（仅桌面平台）
#[tauri::command]
pub async fn select_save_directory(app: AppHandle) -> Result<Option<String>, String> {
    #[cfg(not(target_os = "android"))]
    {
        use tauri_plugin_dialog::DialogExt;

        let folder_path = tokio::task::spawn_blocking(move || {
            app.dialog()
                .file()
                .set_title("选择存储路径")
                .blocking_pick_folder()
        })
        .await
        .map_err(|e| format!("Task failed: {}", e))?;

        Ok(folder_path.and_then(|p| p.as_path().map(|path| path.to_string_lossy().to_string())))
    }

    #[cfg(target_os = "android")]
    {
        // Android: 使用固定路径，不允许用户选择
        let _ = app;
        Ok(Some(crate::platform::android::get_default_storage_path()))
    }
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
    #[cfg(target_os = "android")]
    {
        Ok(crate::platform::android::get_default_storage_path())
    }
    
    #[cfg(not(target_os = "android"))]
    {
        let config = AppConfig::load();
        Ok(config.save_path.to_string_lossy().to_string())
    }
}

/// 获取当前平台名称
#[tauri::command]
pub fn get_platform() -> String {
    #[cfg(target_os = "windows")]
    { "windows".to_string() }
    
    #[cfg(target_os = "macos")]
    { "macos".to_string() }
    
    #[cfg(target_os = "linux")]
    { "linux".to_string() }
    
    #[cfg(target_os = "android")]
    { "android".to_string() }
    
    #[cfg(target_os = "ios")]
    { "ios".to_string() }
    
    #[cfg(not(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "linux",
        target_os = "android",
        target_os = "ios"
    )))]
    { "unknown".to_string() }
}

/// 打开"所有文件访问权限"设置页面（Android）
#[tauri::command]
pub fn open_all_files_access_settings(app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        crate::platform::android::open_manage_storage_settings(&app);
        Ok(())
    }
    
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        Err("此功能仅在 Android 平台可用".to_string())
    }
}
