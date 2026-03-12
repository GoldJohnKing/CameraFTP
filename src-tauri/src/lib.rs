// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

pub mod auto_open;
pub mod commands;
pub mod config;
pub mod constants;
pub mod crypto;
pub mod error;
pub mod file_index;
pub mod ftp;
pub mod network;
pub mod platform;
pub mod utils;

use std::sync::Arc;
use tokio::sync::Mutex;
#[cfg(debug_assertions)]
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tauri::Manager;

use auto_open::AutoOpenService;
use file_index::FileIndexService;
use commands::{
    check_permission_status,
    check_port_available,
    check_server_start_prerequisites,
    check_storage_permission,
    ensure_storage_ready,
    get_autostart_status,
    get_current_file_index,
    get_file_list,
    get_image_exif,
    get_latest_file,
    get_platform,
    get_preview_config,
    get_server_info,
    get_server_status,
    get_storage_info,
    get_storage_path,
    hide_main_window,
    load_config,
    navigate_to_file,
    needs_storage_permission,
    open_all_files_access_settings,
    open_external_link,
    open_folder_select_file,
    open_preview_window,
    quit_application,
    request_all_files_permission,
    save_auth_config,
    save_config,
    select_executable_file,
    select_save_directory,
    set_autostart_command,
    set_preview_config,
    show_main_window,
    start_server,
    stop_server,
    validate_save_path,
    FtpServerState,
    // 文件系统监听
    start_file_watcher,
    stop_file_watcher,
    scan_gallery_images,
    get_latest_image,
};

fn setup_logging() {
    // Debug 模式：写入日志文件 + 控制台
    // Release 模式：不输出任何日志
    #[cfg(debug_assertions)]
    {
        use std::fs;
        use std::path::PathBuf;
        
        // 获取日志目录 - Android 使用外部存储以便用户可以访问
        #[cfg(target_os = "android")]
        let log_dir = PathBuf::from(platform::android::DEFAULT_STORAGE_PATH).join("logs");
        
        #[cfg(target_os = "windows")]
        let log_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("cameraftp/logs");

        let log_file = log_dir.join("app.log");
        let log_file_for_writer = log_file.clone();
        
        // 尝试创建日志目录
        if let Err(e) = fs::create_dir_all(&log_dir) {
            eprintln!("Failed to create log directory {:?}: {}", log_dir, e);
        }

        // 创建文件追加器，失败时回退到 stderr
        let file_appender = tracing_subscriber::fmt::layer()
            .with_writer(move || {
                match std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&log_file_for_writer)
                {
                    Ok(file) => Box::new(file) as Box<dyn std::io::Write + Send + Sync>,
                    Err(_) => Box::new(std::io::stderr()) as Box<dyn std::io::Write + Send + Sync>,
                }
            })
            .with_ansi(false)
            .with_thread_ids(true)
            .with_thread_names(true)
            .with_target(true);

        // 初始化订阅器（同时输出到控制台和文件）
        tracing_subscriber::registry()
            .with(file_appender)
            .with(tracing_subscriber::fmt::layer().with_ansi(false))
            .init();

        tracing::info!(log_file = ?log_file, "Logging initialized (debug mode)");
    }
    
    // Release 模式：不初始化日志系统，不输出任何日志
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logging to file
    setup_logging();

    // 获取平台实例
    let platform = platform::get_platform();
    let is_autostart = platform.is_autostart_mode();

    if is_autostart {
        tracing::info!("Running in autostart mode - window will be hidden");
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(FtpServerState(Arc::new(Mutex::new(None))))
        .manage(Arc::new(FileIndexService::new()))
        .setup(move |app| {
            // 在 setup 中管理 AutoOpenService
            app.manage(AutoOpenService::new(app.handle().clone()));
            // 统一平台初始化（托盘、权限等）
            if let Err(e) = platform.setup(app.handle()) {
                eprintln!("Platform setup failed: {}", e);
            }

            // 初始化 Android 路径（如果是 Android 平台）
            #[cfg(target_os = "android")]
            {
                config::init_android_paths(app.handle());
            }

            // 开机自启模式：隐藏窗口
            if is_autostart {
                platform.hide_window_on_autostart(app.handle());
            }

            // 设置主窗口关闭处理（桌面平台）
            #[cfg(target_os = "windows")]
            setup_window_close_handler(app.handle());

            // 如果是开机启动模式，自动启动服务器
            if is_autostart {
                let state: tauri::State<'_, FtpServerState> = app.state();
                platform.execute_autostart_server(app.handle(), &state.0);
            }

            // 启动后台任务
            spawn_background_tasks(app.handle());

            // 托盘图标状态更新现在由 TrayUpdateHandler 事件驱动
            // 通过 EventBus 监听 StatsUpdated 事件，替代原有的轮询机制

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // 服务器控制
            start_server,
            stop_server,
            get_server_status,
            get_server_info,
            
            // 配置管理
            load_config,
            save_config,
            save_auth_config,
            get_storage_path,
            select_save_directory,
            validate_save_path,
            
            // 网络
            check_port_available,
            
            // 平台
            get_platform,
            
            // 自动启动（Windows）
            set_autostart_command,
            get_autostart_status,
            
            // 应用控制
            quit_application,
            hide_main_window,
            show_main_window,
            
            // Android 权限管理
            open_all_files_access_settings,
            
            // 存储权限（新 API）
            get_storage_info,
            check_permission_status,
            request_all_files_permission,
            ensure_storage_ready,
            check_storage_permission,
            check_server_start_prerequisites,
            needs_storage_permission,

            // 自动预览配置（Windows）
            get_preview_config,
            set_preview_config,
            open_preview_window,
            select_executable_file,
            open_folder_select_file,
            open_external_link,

            // 文件索引
            get_file_list,
            get_current_file_index,
            navigate_to_file,
            get_latest_file,

            // 文件系统监听
            start_file_watcher,
            stop_file_watcher,
            scan_gallery_images,
            get_latest_image,

            // EXIF 信息
            get_image_exif,
        ])
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| {
            eprintln!("Fatal error running Tauri application: {}", e);
            std::process::exit(1);
        });
}

/// 设置主窗口关闭请求处理器（桌面平台）
#[cfg(target_os = "windows")]
fn setup_window_close_handler(app_handle: &tauri::AppHandle) {
    use tauri::Emitter;
    
    if let Some(window) = app_handle.get_webview_window("main") {
        let handle = app_handle.clone();
        window.on_window_event(move |event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                restore_and_focus_window(&handle);
                let _ = handle.emit("window-close-requested", ());
            }
        });
    }
}

/// 恢复并聚焦主窗口
#[cfg(target_os = "windows")]
fn restore_and_focus_window(app_handle: &tauri::AppHandle) {
    if let Some(window) = app_handle.get_webview_window("main") {
        let _ = window.set_skip_taskbar(false);
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// 启动后台任务（文件扫描、文件监听等）
/// 先执行文件扫描，扫描完成后再启动文件监听，避免竞态条件
fn spawn_background_tasks(app_handle: &tauri::AppHandle) {
    let handle = app_handle.clone();

    tauri::async_runtime::spawn(async move {
        // 1. 先执行文件扫描
        let file_index: tauri::State<'_, Arc<FileIndexService>> = handle.state::<Arc<FileIndexService>>();
        if let Err(e) = file_index.scan_directory().await {
            tracing::error!("Failed to scan directory: {}", e);
        }

        // 2. 扫描完成后，启动文件监听
        let file_index_arc = Arc::clone(&file_index);
        match FileIndexService::start_watcher(file_index_arc).await {
            Ok(true) => tracing::info!("File watcher started successfully"),
            Ok(false) => tracing::info!("File watcher not started (unsupported platform)"),
            Err(e) => tracing::error!("Failed to start file watcher: {}", e),
        }
    });
}
