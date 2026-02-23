pub mod commands;
pub mod config;
pub mod error;
pub mod ftp;
pub mod network;
pub mod platform;
pub mod storage_permission;

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tauri::{Manager, Emitter};

use commands::{
    check_port_available, 
    get_autostart_status, 
    get_diagnostic_info, 
    get_network_info, 
    get_platform, 
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
    request_all_files_permission,
};
use ftp::types::ServerStateSnapshot;

fn setup_logging() {
    // 获取日志目录
    let log_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("camera-ftp-companion/logs");

    let _ = fs::create_dir_all(&log_dir);

    let log_file = log_dir.join("app.log");
    let log_file_for_writer = log_file.clone();

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

    // 初始化订阅器
    tracing_subscriber::registry()
        .with(file_appender)
        .with(tracing_subscriber::fmt::layer().with_ansi(false))
        .init();

    tracing::info!(log_file = ?log_file, "Logging initialized");
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logging to file
    setup_logging();

    // 检查是否是开机启动模式（仅在 Windows 上）
    #[cfg(target_os = "windows")]
    let is_autostart = crate::platform::windows::is_autostart_mode();
    
    #[cfg(not(target_os = "windows"))]
    let is_autostart = false;

    if is_autostart {
        tracing::info!("Running in autostart mode - window will be hidden");
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(FtpServerState(Arc::new(Mutex::new(None))))
        .setup(move |app| {
            // 初始化 Android 路径（如果是 Android 平台）
            #[cfg(target_os = "android")]
            {
                config::init_android_paths(app.handle());
            }

            #[cfg(target_os = "windows")]
            {
                if let Err(e) = platform::windows::setup_tray(app.handle()) {
                    eprintln!("Failed to setup tray: {}", e);
                }
            }

            // 获取主窗口并控制显示
            if let Some(window) = app.get_webview_window("main") {
                #[cfg(target_os = "windows")]
                if is_autostart {
                    // 开机启动模式：隐藏窗口（仅 Windows）
                    let _ = window.hide();
                    let _ = window.set_skip_taskbar(true);
                }
                
                // 监听窗口关闭请求（点击X号）
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
            #[cfg(target_os = "windows")]
            if is_autostart {
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

                    let state: tauri::State<'_, FtpServerState> = app_handle.state();

                    match crate::ftp::server_factory::start_ftp_server(&state.0, Default::default()).await {
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
                            tracing::info!("Server auto-started on autostart");

                            // 更新托盘图标为 idle 状态（服务器运行但无连接）
                            if let Err(e) = crate::platform::windows::update_tray_icon(
                                &app_handle,
                                crate::platform::windows::TrayIconState::Idle
                            ) {
                                tracing::warn!("Failed to update tray icon on autostart: {}", e);
                            }
                        }
                        Err(e) => {
                            tracing::error!("Failed to auto-start server: {}", e);
                        }
                    }
                });
            }

            // 启动统计信息推送定时器（优化：只在有变化时推送）
            let app_handle = app.handle().clone();
            let state: tauri::State<'_, FtpServerState> = app.state();
            let state_clone: std::sync::Arc<tokio::sync::Mutex<Option<crate::ftp::FtpServerHandle>>> = state.0.clone();

            #[cfg(target_os = "windows")]
            let tray_app_handle = app.handle().clone();

            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(500));
                let mut last_snapshot: Option<ServerStateSnapshot> = None;

                #[cfg(target_os = "windows")]
                let mut last_tray_state: Option<crate::platform::windows::TrayIconState> = None;

                loop {
                    interval.tick().await;

                    let server_guard: tokio::sync::MutexGuard<'_, Option<crate::ftp::FtpServerHandle>> = state_clone.lock().await;
                    if let Some(server) = server_guard.as_ref() {
                        let snapshot: ServerStateSnapshot = server.get_snapshot().await;

                        // 只在服务器运行且状态变化时推送
                        if snapshot.is_running {
                            let should_emit = match &last_snapshot {
                                None => true,
                                Some(last) => {
                                    last.connected_clients != snapshot.connected_clients
                                        || last.files_received != snapshot.files_received
                                        || last.bytes_received != snapshot.bytes_received
                                        || last.last_file != snapshot.last_file
                                }
                            };

                            if should_emit {
                                let _ = app_handle.emit("stats-update", &snapshot);

                                // 根据连接数更新托盘图标状态（仅在 Windows）
                                #[cfg(target_os = "windows")]
                                {
                                    let new_tray_state = if snapshot.connected_clients > 0 {
                                        crate::platform::windows::TrayIconState::Active
                                    } else {
                                        crate::platform::windows::TrayIconState::Idle
                                    };

                                    // 只在状态变化时更新图标
                                    if last_tray_state != Some(new_tray_state) {
                                        if let Err(e) = crate::platform::windows::update_tray_icon(
                                            &tray_app_handle,
                                            new_tray_state
                                        ) {
                                            tracing::warn!("Failed to update tray icon: {}", e);
                                        }
                                        last_tray_state = Some(new_tray_state);
                                    }
                                }

                                last_snapshot = Some(snapshot);
                            }
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
