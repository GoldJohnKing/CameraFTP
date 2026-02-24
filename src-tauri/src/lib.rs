pub mod commands;
pub mod config;
pub mod error;
pub mod ftp;
pub mod network;
pub mod platform;
pub mod storage_permission;

use std::sync::Arc;
use tokio::sync::Mutex;
#[cfg(debug_assertions)]
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tauri::{Manager, Emitter};

use commands::{
    check_port_available, 
    get_autostart_status, 
    get_diagnostic_info, 
    get_network_info, 
    get_platform, 
    get_server_info,
    get_server_status, 
    get_storage_path,
    hide_main_window, 
    load_config, 
    open_all_files_access_settings, 
    quit_application, 
    save_config, 
    select_save_directory, 
    set_autostart_command, 
    start_server, 
    stop_server, 
    validate_save_path,
    FtpServerState
};
use storage_permission::{
    check_permission_status,
    check_server_start_prerequisites,
    check_storage_permission,
    ensure_storage_ready,
    get_storage_info,
    needs_storage_permission,
    request_all_files_permission,
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
        let log_dir = PathBuf::from("/storage/emulated/0/DCIM/CameraFTP/logs");
        
        #[cfg(not(target_os = "android"))]
        let log_dir = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("camera-ftp-companion/logs");

        let log_file = log_dir.join("app.log");
        let log_file_for_writer = log_file.clone();
        
        // 尝试创建日志目录
        if let Err(e) = fs::create_dir_all(&log_dir) {
            eprintln!("Failed to create log directory {:?}: {}", log_dir, e);
        }

        // 创建文件追加器
        let file_appender = tracing_subscriber::fmt::layer()
            .with_writer(move || {
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&log_file_for_writer)
                    .unwrap_or_else(|_| std::fs::File::create("/dev/null").unwrap())
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
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(FtpServerState(Arc::new(Mutex::new(None))))
        .setup(move |app| {
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

            // 获取主窗口并监听关闭请求
            if let Some(window) = app.get_webview_window("main") {
                let app_handle = app.handle().clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        // 阻止默认关闭行为
                        api.prevent_close();
                        // 发送事件给前端显示确认对话框
                        let _ = app_handle.emit("window-close-requested", ());
                    }
                });
            }

            // 如果是开机启动模式，自动启动服务器
            if is_autostart {
                let state: tauri::State<'_, FtpServerState> = app.state();
                platform.execute_autostart_server(app.handle(), &state.0);
            }

            // 托盘图标状态更新（轻量级轮询，仅更新托盘）
            // 前端统计推送由 EventBus + StatsEventHandler 事件驱动处理
            let app_handle = app.handle().clone();
            let state: tauri::State<'_, FtpServerState> = app.state();
            let state_clone = state.0.clone();
            let platform_ref = platform;

            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));
                let mut last_client_count: usize = 0;

                loop {
                    interval.tick().await;

                    let server_guard = state_clone.lock().await;
                    if let Some(server) = server_guard.as_ref() {
                        let snapshot = server.get_snapshot().await;

                        // 仅在连接数变化时更新托盘图标
                        if snapshot.is_running && snapshot.connected_clients != last_client_count {
                            platform_ref.update_server_state(&app_handle, snapshot.connected_clients as u32);
                            last_client_count = snapshot.connected_clients;
                        }
                    }
                }
            });

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
            get_storage_path,
            select_save_directory,
            validate_save_path,
            
            // 网络
            get_network_info,
            check_port_available,
            
            // 诊断
            get_diagnostic_info,
            get_platform,
            
            // 自动启动（Windows）
            set_autostart_command,
            get_autostart_status,
            
            // 应用控制
            quit_application,
            hide_main_window,
            
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
