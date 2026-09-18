// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::Arc;
use tokio::sync::Mutex;

use crate::ftp::FtpServerSlot;

mod ai_edit;
mod color_grading;
mod config;
mod exif;
mod file_index;
mod server;
mod storage;

/// FTP 服务器状态（使用 Arc<Mutex> 包装以支持异步操作）
///
/// 槽位状态机（FtpServerSlot）：None → Starting（启动权已认领）→ Running，
/// 用于序列化并发的 start_server 调用（UI 按钮与托盘菜单同时触发等场景），
/// 防止并发启动各自绑定端口、产生无法停止的孤儿服务器。
/// 认领/提交/回滚时序见 ftp::server_factory::start_ftp_server。
pub struct FtpServerState(pub Arc<Mutex<FtpServerSlot>>);

// Re-export EXIF info type
pub use exif::ExifInfo;

// Re-export all commands
pub use config::{
    load_config, open_external_link, open_folder_select_file, open_preview_window,
    open_save_directory, save_auth_config, save_config, select_executable_file,
    select_save_directory, update_preview_config,
};

pub use exif::{get_image_exif, get_raw_orientation, inject_exif_orientation};

pub use file_index::{get_current_file_index, get_file_list, get_latest_image, navigate_to_file};

pub use server::{
    check_port_available, get_server_runtime_state, hide_main_window, quit_application,
    show_main_window, start_server, stop_server,
};

pub use ai_edit::{cancel_ai_edit, enqueue_ai_edit};

pub use color_grading::{
    apply_color_grading_preview, begin_color_grading_preview, cancel_color_grading,
    end_color_grading_preview, enqueue_color_grading, get_color_grading_presets,
};

pub use storage::{
    check_permission_status, check_server_start_prerequisites, ensure_storage_ready,
    get_autostart_status, get_platform, get_storage_info, set_autostart_command,
};
