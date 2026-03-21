// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::Arc;
use tokio::sync::Mutex;

use crate::ftp::FtpServerHandle;

mod config;
mod exif;
mod file_index;
mod server;
mod storage;

/// FTP 服务器状态（使用 Arc<Mutex> 包装以支持异步操作）
pub struct FtpServerState(pub Arc<Mutex<Option<FtpServerHandle>>>);

impl FtpServerState {
    /// Helper method to access the server with a synchronous closure.
    /// Returns None if server is not running.
    pub async fn with_server<F, T>(&self, f: F) -> Option<T>
    where
        F: FnOnce(&FtpServerHandle) -> T,
    {
        let guard = self.0.lock().await;
        guard.as_ref().map(f)
    }
}

// Re-export EXIF info type
pub use exif::ExifInfo;

// Re-export all commands
pub use config::{
    get_storage_path,
    load_config,
    open_external_link,
    open_folder_select_file,
    open_preview_window,
    save_auth_config,
    save_config,
    select_executable_file,
    select_save_directory,
    update_preview_config,
    validate_save_path,
};

pub use exif::get_image_exif;

pub use file_index::{
    get_current_file_index,
    get_file_list,
    get_latest_file,
    navigate_to_file,
    start_file_watcher,
    stop_file_watcher,
    scan_gallery_images,
    get_latest_image,
};

pub use server::{
    check_port_available,
    hide_main_window,
    quit_application,
    show_main_window,
    start_server,
    stop_server,
    get_server_info,
    get_server_status,
};

pub use storage::{
    check_permission_status,
    check_server_start_prerequisites,
    check_storage_permission,
    ensure_storage_ready,
    get_autostart_status,
    get_platform,
    get_storage_info,
    needs_storage_permission,
    open_all_files_access_settings,
    request_all_files_permission,
    set_autostart_command,
};
