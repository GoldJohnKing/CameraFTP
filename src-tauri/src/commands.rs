use tauri::{command, AppHandle, Emitter, Manager, State};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info, instrument};

use crate::auto_open::AutoOpenService;
use crate::config::{AppConfig, PreviewWindowConfig};
use crate::error::AppError;
use crate::ftp::types::{ServerInfo, ServerStateSnapshot};
use crate::ftp::FtpServerHandle;
use crate::network::NetworkManager;
use crate::platform::{get_platform as get_platform_service, PermissionStatus, ServerStartCheckResult, StorageInfo};

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

// ============================================================================
// 存储权限管理命令（从 storage_permission.rs 迁移）
// ============================================================================

/// 获取固定存储路径信息
#[tauri::command]
pub async fn get_storage_info() -> Result<StorageInfo, AppError> {
    Ok(get_platform_service().get_storage_info())
}

/// 检查权限状态
#[tauri::command]
pub async fn check_permission_status() -> Result<PermissionStatus, AppError> {
    Ok(get_platform_service().check_permission_status())
}

/// 请求"所有文件访问权限"
#[tauri::command]
pub async fn request_all_files_permission(app: AppHandle) -> Result<(), AppError> {
    let platform = get_platform_service();
    
    platform
        .request_all_files_permission(&app)
        .map_err(AppError::StoragePermissionError)?;

    // 如果返回 false，说明需要用户去设置页面授权
    // 这里我们不返回错误，让前端决定如何处理
    Ok(())
}

/// 确保存储目录存在且可写
#[tauri::command]
pub async fn ensure_storage_ready() -> Result<String, AppError> {
    get_platform_service()
        .ensure_storage_ready()
        .map_err(AppError::StoragePermissionError)
}

/// 检查存储权限
#[tauri::command]
pub async fn check_storage_permission() -> Result<bool, AppError> {
    Ok(get_platform_service().check_permission_status().has_all_files_access)
}

/// 检查服务器启动前提条件
#[tauri::command]
pub async fn check_server_start_prerequisites() -> Result<ServerStartCheckResult, AppError> {
    Ok(get_platform_service().check_server_start_prerequisites())
}

/// 检查是否需要存储权限（用于前端 UI 判断）
#[tauri::command]
pub async fn needs_storage_permission() -> bool {
    get_platform_service().needs_storage_permission()
}

// ============================================================================
// 自动预览配置命令（Windows）
// ============================================================================

#[derive(Clone, serde::Serialize)]
struct PreviewEvent {
    file_path: String,
    bring_to_front: bool,
}

/// 获取预览窗口配置
#[tauri::command]
pub async fn get_preview_config(
    auto_open: State<'_, AutoOpenService>,
) -> Result<PreviewWindowConfig, AppError> {
    Ok(auto_open.get_config().await)
}

/// 设置预览窗口配置
#[tauri::command]
pub async fn set_preview_config(
    auto_open: State<'_, AutoOpenService>,
    config: PreviewWindowConfig,
) -> Result<(), AppError> {
    auto_open.update_config(config).await;
    Ok(())
}

/// 手动打开预览窗口
#[tauri::command]
pub async fn open_preview_window(
    app: AppHandle,
    file_path: String,
) -> Result<(), AppError> {
    // 检查预览窗口是否已存在
    if let Some(window) = app.get_webview_window("preview") {
        // 窗口已存在，发送事件更新图片
        let event = PreviewEvent {
            file_path,
            bring_to_front: true,
        };
        window.emit("preview-image", event)
            .map_err(|e| AppError::Other(format!("Failed to emit preview event: {}", e)))?;
        
        // 将窗口带到前台
        window.set_focus()
            .map_err(|e| AppError::Other(format!("Failed to focus window: {}", e)))?;
    } else {
        // 创建新窗口
        let window = tauri::WebviewWindowBuilder::new(
            &app,
            "preview",
            tauri::WebviewUrl::App("/preview".into())
        )
        .title("图片预览")
        .inner_size(1024.0, 768.0)
        .center()
        .resizable(true)
        .visible(true)
        .build()
        .map_err(|e| AppError::Other(format!("Failed to create preview window: {}", e)))?;
        
        // 延迟发送事件，确保窗口已加载
        let event = PreviewEvent {
            file_path,
            bring_to_front: true,
        };
        
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            let _ = window.emit("preview-image", event);
        });
    }

    Ok(())
}

/// 选择可执行文件（用于自定义打开程序）
#[tauri::command]
pub async fn select_executable_file(app: AppHandle) -> Result<Option<String>, AppError> {
    #[cfg(not(target_os = "android"))]
    {
        use tauri_plugin_dialog::DialogExt;

        let file_path = tokio::task::spawn_blocking(move || {
            app.dialog()
                .file()
                .set_title("选择程序")
                .add_filter("可执行文件", &["exe"])
                .blocking_pick_file()
        })
        .await
        .map_err(|e| AppError::Other(format!("Task failed: {}", e)))?;

        return Ok(file_path.and_then(|p| p.as_path().map(|path| path.to_string_lossy().to_string())));
    }

    #[cfg(target_os = "android")]
    {
        Ok(None)
    }
}
