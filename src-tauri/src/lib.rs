pub mod commands;
pub mod config;
pub mod ftp;
pub mod network;
pub mod platform;

use std::sync::Arc;
use tokio::sync::Mutex;

use commands::{FtpServerState, start_server, stop_server, get_server_status, 
               get_network_info, load_config, save_config, check_port_available};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}