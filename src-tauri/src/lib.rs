pub mod commands;
pub mod config;
pub mod error;
pub mod ftp;
pub mod network;
pub mod platform;

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tauri::{Manager, Emitter};

use commands::{check_port_available, get_autostart_status, get_diagnostic_info, get_network_info, get_server_status, load_config, quit_application, save_config, set_autostart_command, start_server, stop_server, FtpServerState};
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

    // 检查是否是开机启动模式
    let is_autostart = cfg!(target_os = "windows") 
        && crate::platform::windows::is_autostart_mode();

    if is_autostart {
        tracing::info!("Running in autostart mode - window will be hidden");
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(FtpServerState(Arc::new(Mutex::new(None))))
        .setup(move |app| {
            #[cfg(target_os = "windows")]
            {
                if let Err(e) = platform::windows::setup_tray(app.handle()) {
                    eprintln!("Failed to setup tray: {}", e);
                }
            }

            // 获取主窗口并控制显示
            if let Some(window) = app.get_webview_window("main") {
                if is_autostart {
                    // 开机启动模式：隐藏窗口
                    let _ = window.hide();
                    let _ = window.set_skip_taskbar(true);
                }
            }

            // 如果是开机启动模式，自动启动服务器
            if is_autostart {
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    // 延迟一点时间确保配置加载完成
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

                    let state: tauri::State<'_, FtpServerState> = app_handle.state();
                    let mut server_guard = state.0.lock().await;

                    if server_guard.is_none() {
                        let config = crate::config::AppConfig::load();
                        match crate::ftp::FtpServerHandle::start(&config.ftp).await {
                            Ok(handle) => {
                                *server_guard = Some(handle);
                                let _ = app_handle.emit("server-started", ());
                                tracing::info!("Server auto-started on autostart");
                            }
                            Err(e) => {
                                tracing::error!("Failed to auto-start server: {}", e);
                            }
                        }
                    }
                });
            }

            // 启动统计信息推送定时器（优化：只在有变化时推送）
            let app_handle = app.handle().clone();
            let state: tauri::State<'_, FtpServerState> = app.state();
            let state_clone: std::sync::Arc<tokio::sync::Mutex<Option<crate::ftp::FtpServerHandle>>> = state.0.clone();

            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(500)); // 改为500ms
                let mut last_snapshot: Option<ServerStateSnapshot> = None;

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
                                last_snapshot = Some(snapshot);
                            }
                        }
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            start_server,
            stop_server,
            get_server_status,
            get_network_info,
            load_config,
            save_config,
            check_port_available,
            get_diagnostic_info,
            set_autostart_command,
            get_autostart_status,
            quit_application,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
