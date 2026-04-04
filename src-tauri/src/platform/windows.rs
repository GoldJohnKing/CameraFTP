// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::env;
use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, Wry};
use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconEvent};
use winreg::enums::*;
use winreg::RegKey;

use crate::config_service::ConfigService;
use crate::constants::{
    SERVER_READY_TIMEOUT_SECS, AUTOSTART_DELAY_MS,
};
use crate::ftp::types::ServerStateSnapshot;
use super::traits::PlatformService;
use super::types::{StorageInfo, PermissionStatus};

/// 托盘菜单状态 - 存储菜单项引用用于动态更新
pub struct TrayMenuState {
    pub start_item: MenuItem<Wry>,
    pub stop_item: MenuItem<Wry>,
}

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

/// Show and focus the main window
fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        // 重置 skip_taskbar 状态，确保窗口能正常显示在任务栏
        let _ = window.set_skip_taskbar(false);
        // 如果窗口被最小化，先恢复
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// 更新托盘图标
/// 
/// # Arguments
/// * `app` - Tauri 应用句柄
/// * `state` - 托盘图标状态（Stopped/Idle/Active）
pub fn update_tray_icon(app: &AppHandle, state: TrayIconState) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(tray) = app.tray_by_id("main") {
        let icon_data = match state {
            TrayIconState::Stopped => TRAY_STOPPED_PNG,
            TrayIconState::Idle => TRAY_IDLE_PNG,
            TrayIconState::Active => TRAY_ACTIVE_PNG,
        };
        
        let icon = create_icon_from_bytes(icon_data)?;
        tray.set_icon(Some(icon))?;
    }
    Ok(())
}

/// 更新托盘菜单项状态
/// 
/// # Arguments
/// * `app` - Tauri 应用句柄
/// * `server_running` - 服务器是否正在运行
pub fn update_tray_menu(app: &AppHandle, server_running: bool) -> Result<(), Box<dyn std::error::Error>> {
    // 从 State 获取菜单项引用
    if let Some(state) = app.try_state::<TrayMenuState>() {
        state.start_item.set_enabled(!server_running)?;
        state.stop_item.set_enabled(server_running)?;
    }
    Ok(())
}

pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    // 创建菜单项
    let show_i = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
    // 初始状态：服务器未运行，所以"启动"启用，"停止"禁用
    let start_i = MenuItem::with_id(app, "start", "启动服务器", true, None::<&str>)?;
    let stop_i = MenuItem::with_id(app, "stop", "停止服务器", false, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit_i = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

    let menu = Menu::with_items(app, &[
        &show_i,
        &start_i,
        &stop_i,
        &separator,
        &quit_i,
    ])?;

    // 保存菜单项引用到 State，用于后续动态更新
    app.manage(TrayMenuState {
        start_item: start_i,
        stop_item: stop_i,
    });

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
                show_main_window(app);
            }
        })
        .on_menu_event(move |app: &AppHandle, event: MenuEvent| {
            match event.id.as_ref() {
                "show" => {
                    show_main_window(app);
                }
                "start" => {
                    // 发送事件给前端，由前端统一处理启动逻辑
                    // 这样可以确保前端状态正确同步
                    let _ = app.emit("tray-start-server", ());
                    tracing::info!("Emitted tray-start-server event");
                }
                "stop" => {
                    // 发送事件给前端，由前端统一处理停止逻辑
                    let _ = app.emit("tray-stop-server", ());
                    tracing::info!("Emitted tray-stop-server event");
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

fn load_config_from_service(app: &AppHandle) -> Result<crate::config::AppConfig, String> {
    let config_service = app.state::<Arc<ConfigService>>();
    config_service
        .get()
        .map_err(|e| format!("读取配置失败: {}", e))
}

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
    
    fn ensure_storage_ready(&self, app: &AppHandle) -> Result<String, String> {
        let config = load_config_from_service(app)?;
        let save_path = config.save_path.clone();

        // 确保目录存在
        if !save_path.exists() {
            std::fs::create_dir_all(&save_path).map_err(|e| {
                format!("无法创建保存目录 '{}': {}", save_path.display(), e)
            })?;
        }

        // 验证可写性
        let test_file = save_path.join(".write_test");
        match std::fs::write(&test_file, b"test") {
            Ok(_) => {
                let _ = std::fs::remove_file(&test_file);
                Ok(save_path.to_string_lossy().to_string())
            }
            Err(e) => {
                Err(format!(
                    "保存目录 '{}' 没有写入权限 ({})",
                    save_path.display(),
                    e
                ))
            }
        }
    }
    
    fn on_server_started(&self, app: &AppHandle) {
        if let Err(e) = update_tray_icon(app, TrayIconState::Idle) {
            tracing::warn!("Failed to update tray icon: {}", e);
        }
        if let Err(e) = update_tray_menu(app, true) {
            tracing::warn!("Failed to update tray menu: {}", e);
        }
    }
    
    fn on_server_stopped(&self, app: &AppHandle) {
        if let Err(e) = update_tray_icon(app, TrayIconState::Stopped) {
            tracing::warn!("Failed to update tray icon: {}", e);
        }
        if let Err(e) = update_tray_menu(app, false) {
            tracing::warn!("Failed to update tray menu: {}", e);
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

    fn sync_android_service_state(&self, _app: &AppHandle, _snapshot: &ServerStateSnapshot) {}

    // ========== 开机自启相关 ==========

    fn set_autostart(&self, enable: bool) -> Result<(), String> {
        set_autostart(enable).map_err(|e| format!("设置开机自启失败: {}", e))
    }

    fn is_autostart_enabled(&self) -> Result<bool, String> {
        is_autostart_enabled().map_err(|e| format!("获取自启状态失败: {}", e))
    }

    fn is_autostart_mode(&self) -> bool {
        is_autostart_mode()
    }

    fn hide_window_on_autostart(&self, app: &AppHandle) {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.hide();
            let _ = window.set_skip_taskbar(true);
        }
    }

    fn get_default_storage_path(&self) -> std::path::PathBuf {
        dirs::picture_dir().unwrap_or_else(|| std::path::PathBuf::from("./pictures"))
    }

    fn check_server_start_prerequisites(&self) -> super::types::ServerStartCheckResult {
        // Windows 平台无需权限检查，直接返回可启动
        let storage_info = self.get_storage_info();
        super::types::ServerStartCheckResult {
            can_start: true,
            reason: None,
            storage_info: Some(storage_info),
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
            // 短暂延迟，让应用完全初始化（而非等待服务器启动的1秒）
            tokio::time::sleep(tokio::time::Duration::from_millis(AUTOSTART_DELAY_MS)).await;

            match crate::ftp::server_factory::start_ftp_server(&state_clone, Default::default(), app_handle.clone()).await {
                Ok(ctx) => {
                    tracing::info!("FTP server auto-started on {}:{}", ctx.ip, ctx.port);

                    // 先启动事件处理器（获取就绪信号）
                    // 注意：传递 event_bus 的引用，不要克隆，以确保处理器和服务器共享同一个状态通道
                    let ready_rx = crate::ftp::server_factory::spawn_event_processor(
                        app_handle.clone(),
                        &ctx.event_bus,
                    );

                    // 等待事件处理器就绪（而非固定延迟）
                    match tokio::time::timeout(
                        tokio::time::Duration::from_secs(SERVER_READY_TIMEOUT_SECS),
                        ready_rx
                    ).await {
                        Ok(_) => tracing::debug!("Event processor ready"),
                        Err(_) => tracing::warn!("Event processor ready timeout, continuing anyway"),
                    }

                    let file_index = app_handle.state::<Arc<crate::file_index::FileIndexService>>();
                    file_index.set_event_bus(ctx.event_bus).await;
                }
                Err(e) => {
                    tracing::error!("Failed to auto-start server: {}", e);
                }
            }
        });
    }

    // ========== 窗口与UI相关 ==========

    fn hide_main_window(&self, app: &AppHandle) -> Result<(), String> {
        if let Some(window) = app.get_webview_window("main") {
            window.hide()
                .map_err(|e| format!("隐藏窗口失败: {}", e))
        } else {
            Err("主窗口不存在".to_string())
        }
    }

    fn show_main_window(&self, app: &AppHandle) -> Result<(), String> {
        tracing::info!("Showing and focusing main window");
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.set_skip_taskbar(false);
            let _ = window.unminimize();
            let _ = window.show();
            let _ = window.set_focus();
        }
        Ok(())
    }

    fn select_save_directory(&self, _app: &AppHandle) -> Result<Option<String>, String> {
        // Windows 平台通过前端对话框选择，这里返回 None 表示使用前端选择
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn windows_autostart_no_longer_reemits_server_started_event() {
        let source = include_str!("windows.rs");

        assert!(!source.contains("ctx.event_bus\n                        .emit_server_started"));
        assert!(source.contains("file_index.set_event_bus(ctx.event_bus).await;"));
    }
}
