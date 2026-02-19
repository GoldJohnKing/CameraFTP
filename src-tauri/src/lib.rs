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

use commands::{check_port_available, get_diagnostic_info, get_network_info, get_server_status, load_config, save_config, start_server, stop_server, FtpServerState};
use ftp::ServerStateSnapshot;

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
        .with_ansi(false);
    
    // 初始化订阅器
    tracing_subscriber::registry()
        .with(file_appender)
        .with(tracing_subscriber::fmt::layer().with_ansi(false))
        .init();
    
    tracing::info!("Logging initialized. Log file: {:?}", log_file);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logging to file
    setup_logging();
    
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(FtpServerState(Arc::new(Mutex::new(None))))
        .setup(|app| {
            #[cfg(target_os = "windows")]
            {
                if let Err(e) = platform::windows::setup_tray(app.handle()) {
                    eprintln!("Failed to setup tray: {}", e);
                }
            }
            
            // 启动统计信息推送定时器
            let app_handle = app.handle().clone();
            let state: tauri::State<'_, FtpServerState> = app.state();
            let state_clone: std::sync::Arc<tokio::sync::Mutex<Option<crate::ftp::FtpServer>>> = state.0.clone();
            
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));
                loop {
                    interval.tick().await;
                    
                    let server_guard: tokio::sync::MutexGuard<'_, Option<crate::ftp::FtpServer>> = state_clone.lock().await;
                    if let Some(server) = server_guard.as_ref() {
                        let snapshot: ServerStateSnapshot = server.state_snapshot();
                        // 只推送有变化时的状态，或者服务器正在运行时
                        if snapshot.is_running {
                            let _ = app_handle.emit("stats-update", snapshot);
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}