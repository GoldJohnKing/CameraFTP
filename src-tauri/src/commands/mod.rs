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

// Re-export EXIF info type
pub use exif::ExifInfo;

// Re-export all commands
pub use config::{
    load_config,
    open_external_link,
    open_folder_select_file,
    open_preview_window,
    save_auth_config,
    save_config,
    select_executable_file,
    select_save_directory,
    update_preview_config,
};

pub use exif::get_image_exif;

pub use file_index::{
    get_current_file_index,
    get_file_list,
    get_latest_file,
    get_latest_image,
    navigate_to_file,
};

pub use server::{
    check_port_available,
    hide_main_window,
    quit_application,
    show_main_window,
    start_server,
    stop_server,
    get_server_runtime_state,
};

pub use storage::{
    check_permission_status,
    check_server_start_prerequisites,
    ensure_storage_ready,
    get_autostart_status,
    get_platform,
    get_storage_info,
    request_all_files_permission,
    set_autostart_command,
};
