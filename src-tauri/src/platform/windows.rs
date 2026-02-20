use tauri::{AppHandle, Manager};
use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconEvent};

/// 绿色托盘图标数据（编译时嵌入）
const TRAY_ACTIVE_PNG: &[u8] = include_bytes!("../../icons/tray-active.png");

/// 从嵌入的PNG数据创建图标
fn create_green_icon() -> Result<tauri::image::Image<'static>, Box<dyn std::error::Error>> {
    let img = image::load_from_memory_with_format(TRAY_ACTIVE_PNG, image::ImageFormat::Png)?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    
    let icon = tauri::image::Image::new_owned(rgba.into_raw(), width, height);
    Ok(icon)
}

/// 更新托盘图标
/// - running: true 使用绿色图标（服务器运行中）
/// - running: false 使用默认蓝色图标（服务器停止）
pub fn update_tray_icon(app: &AppHandle, running: bool) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(tray) = app.tray_by_id("main") {
        if running {
            // 使用绿色图标（服务器运行）- 从嵌入数据创建
            let icon = create_green_icon()?;
            tray.set_icon(Some(icon))?;
            tracing::info!("Tray icon updated to green (server running)");
        } else {
            // 使用默认蓝色图标（服务器停止）
            if let Some(default_icon) = app.default_window_icon() {
                tray.set_icon(Some(default_icon.clone()))?;
                tracing::info!("Tray icon updated to blue (server stopped)");
            }
        }
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

    let _tray = tauri::tray::TrayIconBuilder::with_id("main")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .icon(app.default_window_icon().unwrap().clone())
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
