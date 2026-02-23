use tauri::{AppHandle, Manager};
use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconEvent};
use super::traits::PlatformService;
use super::types::{StorageInfo, PermissionStatus};
use std::sync::Arc;
use tokio::sync::Mutex;

/// 托盘图标状态枚举
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrayIconState {
    /// 服务器未启动 - 红色圆点
    Stopped,
    /// 服务器运行但无设备连接 - 黄色圆点
    Idle,
    /// 服务器运行且有设备连接 - 绿色圆点
    Active,
}

/// 托盘图标数据（编译时嵌入）
const TRAY_STOPPED_PNG: &[u8] = include_bytes!("../../icons/tray-stopped.png");
const TRAY_IDLE_PNG: &[u8] = include_bytes!("../../icons/tray-idle.png");
const TRAY_ACTIVE_PNG: &[u8] = include_bytes!("../../icons/tray-active.png");

/// 从嵌入的PNG数据创建图标
fn create_icon_from_bytes(data: &[u8]) -> Result<tauri::image::Image<'static>, Box<dyn std::error::Error>> {
    let img = image::load_from_memory_with_format(data, image::ImageFormat::Png)?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    
    let icon = tauri::image::Image::new_owned(rgba.into_raw(), width, height);
    Ok(icon)
}

/// 更新托盘图标
/// 
/// # Arguments
/// * `app` - Tauri 应用句柄
/// * `state` - 托盘图标状态（Stopped/Idle/Active）
pub fn update_tray_icon(app: &AppHandle, state: TrayIconState) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(tray) = app.tray_by_id("main") {
        let (icon_data, state_name) = match state {
            TrayIconState::Stopped => (TRAY_STOPPED_PNG, "stopped (red dot)"),
            TrayIconState::Idle => (TRAY_IDLE_PNG, "idle (yellow dot)"),
            TrayIconState::Active => (TRAY_ACTIVE_PNG, "active (green dot)"),
        };
        
        let icon = create_icon_from_bytes(icon_data)?;
        tray.set_icon(Some(icon))?;
        tracing::info!("Tray icon updated to {}", state_name);
    }
    Ok(())
}

pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    // 创建菜单项
    let show_i = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
    let start_i = MenuItem::with_id(app, "start", "启动服务器", true, None::<&str>)?;
    let stop_i = MenuItem::with_id(app, "stop", "停止服务器", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit_i = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[
        &show_i,
        &start_i,
        &stop_i,
        &separator,
        &quit_i,
    ])?;

    // 初始状态使用 stopped 图标（红色圆点）
    let initial_icon = create_icon_from_bytes(TRAY_STOPPED_PNG)?;

    let _tray = tauri::tray::TrayIconBuilder::with_id("main")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .icon(initial_icon)
        .icon_as_template(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .on_menu_event(move |app: &AppHandle, event: MenuEvent| {
            match event.id.as_ref() {
                "show" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                "start" => {
                    let app_handle = app.clone();
                    tauri::async_runtime::spawn(async move {
                        match crate::commands::start_server(app_handle.state(), app_handle.clone()).await {
                            Ok(info) => {
                                tracing::info!("Server started from tray: {:?}", info);
                            }
                            Err(e) => {
                                tracing::error!("Failed to start server from tray: {}", e);
                            }
                        }
                    });
                }
                "stop" => {
                    let app_handle = app.clone();
                    tauri::async_runtime::spawn(async move {
                        match crate::commands::stop_server(app_handle.state(), app_handle.clone()).await {
                            Ok(_) => {
                                tracing::info!("Server stopped from tray");
                            }
                            Err(e) => {
                                tracing::error!("Failed to stop server from tray: {}", e);
                            }
                        }
                    });
                }
                "quit" => {
                    // 托盘菜单退出直接退出程序，不显示确认弹窗
                    app.exit(0);
                }
                _ => {}
            }
        })
        .build(app)?;

    tracing::info!("System tray initialized successfully");
    Ok(())
}

use winreg::enums::*;
use winreg::RegKey;
use std::env;

const AUTOSTART_REGISTRY_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const APP_REGISTRY_NAME: &str = "CameraFtpCompanion";

/// 设置开机自启
pub fn set_autostart(enable: bool) -> Result<(), Box<dyn std::error::Error>> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey(AUTOSTART_REGISTRY_KEY)?;
    
    if enable {
        let exe_path = env::current_exe()?;
        let exe_path_str = exe_path.to_string_lossy();
        let value = format!("\"{}\" --autostart", exe_path_str);
        key.set_value(APP_REGISTRY_NAME, &value)?;
        tracing::info!("Autostart enabled: {}", value);
    } else {
        key.delete_value(APP_REGISTRY_NAME)?;
        tracing::info!("Autostart disabled");
    }
    
    Ok(())
}

/// 检查是否已设置开机自启
pub fn is_autostart_enabled() -> Result<bool, Box<dyn std::error::Error>> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu.open_subkey(AUTOSTART_REGISTRY_KEY)?;
    
    match key.get_value::<String, _>(APP_REGISTRY_NAME) {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

/// 检查当前是否是通过开机自启启动的
pub fn is_autostart_mode() -> bool {
    env::args().any(|arg| arg == "--autostart")
}

/// Windows 平台实现
pub struct WindowsPlatform;

impl PlatformService for WindowsPlatform {
    fn name(&self) -> &'static str {
        "windows"
    }
    
    fn setup(&self, app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
        setup_tray(app)?;
        tracing::info!("Windows platform initialized");
        Ok(())
    }
    
    fn get_storage_info(&self) -> StorageInfo {
        StorageInfo {
            display_name: "本地存储".to_string(),
            path: String::new(),
            exists: true,
            writable: true,
            has_all_files_access: true,
        }
    }
    
    fn check_permission_status(&self) -> PermissionStatus {
        PermissionStatus {
            has_all_files_access: true,
            needs_user_action: false,
        }
    }
    
    fn ensure_storage_ready(&self) -> Result<String, String> {
        Ok(String::new())
    }
    
    fn on_server_started(&self, app: &AppHandle) {
        if let Err(e) = update_tray_icon(app, TrayIconState::Idle) {
            tracing::warn!("Failed to update tray icon: {}", e);
        }
    }
    
    fn on_server_stopped(&self, app: &AppHandle) {
        if let Err(e) = update_tray_icon(app, TrayIconState::Stopped) {
            tracing::warn!("Failed to update tray icon: {}", e);
        }
    }
    
    fn update_server_state(&self, app: &AppHandle, connected_clients: u32) {
        let state = if connected_clients > 0 {
            TrayIconState::Active
        } else {
            TrayIconState::Idle
        };
        if let Err(e) = update_tray_icon(app, state) {
            tracing::warn!("Failed to update tray icon: {}", e);
        }
    }

    // ========== 开机自启相关 ==========

    fn is_autostart_mode(&self) -> bool {
        is_autostart_mode()
    }

    fn hide_window_on_autostart(&self, app: &AppHandle) {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.hide();
            let _ = window.set_skip_taskbar(true);
        }
    }

    fn execute_autostart_server(
        &self,
        app: &AppHandle,
        state: &Arc<Mutex<Option<crate::ftp::FtpServerHandle>>>,
    ) {
        let app_handle = app.clone();
        let state_clone = state.clone();

        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

            match crate::ftp::server_factory::start_ftp_server(&state_clone, Default::default()).await {
                Ok(ctx) => {
                    tracing::info!("FTP server auto-started on {}:{}", ctx.ip, ctx.port);

                    // 启动事件处理器
                    crate::ftp::server_factory::spawn_event_processor(
                        app_handle.clone(),
                        ctx.event_bus,
                        500
                    );

                    // 发送事件给前端
                    crate::ftp::server_factory::emit_server_started(&app_handle, &ctx.ip, ctx.port);

                    // 更新托盘图标为 idle 状态
                    if let Err(e) = update_tray_icon(&app_handle, TrayIconState::Idle) {
                        tracing::warn!("Failed to update tray icon on autostart: {}", e);
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to auto-start server: {}", e);
                }
            }
        });
    }
}
