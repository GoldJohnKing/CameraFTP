// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::traits::PlatformService;
use super::types::{PermissionStatus, StorageInfo};
use crate::constants::ANDROID_DCIM_PATH;
use crate::utils::fs::is_path_writable;
use tauri::{AppHandle, Emitter};
use tracing::{debug, error, info};

// 重新导出常量（使用 crate 路径避免导入警告）
pub use crate::constants::ANDROID_DEFAULT_STORAGE_PATH as DEFAULT_STORAGE_PATH;
pub use crate::constants::ANDROID_STORAGE_DISPLAY_NAME as STORAGE_DISPLAY_NAME;

/// 获取存储路径信息
pub fn get_storage_info() -> StorageInfo {
    let path = DEFAULT_STORAGE_PATH;
    let path_buf = std::path::PathBuf::from(path);

    let exists = path_buf.exists();
    let writable = if exists {
        validate_path_writable(path)
    } else {
        false
    };

    // 检查权限：如果能写入，就认为有权限
    let has_all_files_access = writable || (exists && can_write_to_dcim());

    StorageInfo {
        display_name: STORAGE_DISPLAY_NAME.to_string(),
        path: path.to_string(),
        exists,
        writable,
        has_all_files_access,
    }
}

/// 检查权限状态
pub fn check_permission_status() -> PermissionStatus {
    let has_access = check_media_store_permission();
    PermissionStatus {
        has_all_files_access: has_access,
        needs_user_action: !has_access,
    }
}

/// 检查是否有媒体存储权限
/// 权限检查现在通过 Kotlin bridge 完成；假设如果可以查询 MediaStore 就已授权
fn check_media_store_permission() -> bool {
    // Permission check now done via Kotlin bridge; assume granted if we can query MediaStore
    true
}

/// 检查 DCIM 目录是否可写（用于判断所有文件访问权限）
fn can_write_to_dcim() -> bool {
    let dcim_path = std::path::Path::new(ANDROID_DCIM_PATH);
    if !dcim_path.exists() {
        debug!("DCIM path does not exist");
        return false;
    }
    let writable = is_path_writable(dcim_path);
    if writable {
        debug!("All files access permission: granted (DCIM writable)");
    } else {
        debug!("All files access permission: denied (DCIM not writable)");
    }
    writable
}

/// 验证路径是否可写
fn validate_path_writable(path: &str) -> bool {
    let path_buf = std::path::PathBuf::from(path);

    // 如果路径不存在，尝试创建
    if !path_buf.exists() {
        debug!("Path does not exist, attempting to create: {:?}", path_buf);
        match std::fs::create_dir_all(&path_buf) {
            Ok(_) => {
                info!("Successfully created directory: {:?}", path_buf);
            }
            Err(e) => {
                error!("Failed to create directory {:?}: {}", path_buf, e);
                return false;
            }
        }
    }

    // 确保是目录
    if !path_buf.is_dir() {
        error!("Path exists but is not a directory: {:?}", path_buf);
        return false;
    }

    // 使用共享辅助函数检查可写性
    let writable = is_path_writable(&path_buf);
    if writable {
        debug!("Path is writable: {:?}", path_buf);
    } else {
        error!("Path is not writable: {:?}", path_buf);
    }
    writable
}

/// 确保存储目录存在且可写
/// 前端通过 PermissionDialog 处理权限检查，这里只负责创建目录
pub fn ensure_storage_ready() -> Result<String, String> {
    let path = DEFAULT_STORAGE_PATH;
    let path_buf = std::path::PathBuf::from(path);

    // 创建目录（如果不存在）
    // 前端已处理权限检查，这里直接尝试创建目录
    if !path_buf.exists() {
        std::fs::create_dir_all(&path_buf).map_err(|e| format!("无法创建存储目录: {}", e))?;
        info!("Created storage directory: {}", path);
    }

    Ok(path.to_string())
}

/// 打开存储权限设置页面
pub fn open_storage_permission_settings(app: &AppHandle) {
    let _ = app.emit("android-open-storage-permission-settings", ());
    info!("Requesting READ_MEDIA_IMAGES permission");
}

/// Android 平台实现
pub struct AndroidPlatform;

impl PlatformService for AndroidPlatform {
    fn name(&self) -> &'static str {
        "android"
    }

    fn setup(&self, _app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
        tracing::info!("Android platform initialized");
        Ok(())
    }

    fn get_storage_info(&self) -> StorageInfo {
        get_storage_info()
    }

    fn check_permission_status(&self) -> PermissionStatus {
        check_permission_status()
    }

    fn ensure_storage_ready(&self) -> Result<String, String> {
        ensure_storage_ready()
    }

    fn check_server_start_prerequisites(&self) -> super::types::ServerStartCheckResult {
        // Android 平台：前端通过 PermissionDialog 处理权限检查
        // 这里始终返回可启动，因为权限检查在前端完成
        // 前端会确保用户已授权所有文件访问权限后才允许启动服务器
        let storage_info = self.get_storage_info();
        super::types::ServerStartCheckResult {
            can_start: true,
            reason: None,
            storage_info: Some(storage_info),
        }
    }

    // Note: on_server_started/on_server_stopped use default empty implementation
    // Notification is managed via update_server_state() which is called from frontend

    fn update_server_state(&self, app: &AppHandle, connected_clients: u32) {
        // Emit event to Android for notification update
        let _ = app.emit(
            "android-service-state-update",
            serde_json::json!({
                "connected_clients": connected_clients,
            }),
        );
    }

    fn get_storage_path(&self) -> Result<String, String> {
        Ok(DEFAULT_STORAGE_PATH.to_string())
    }

    fn get_default_storage_path(&self) -> std::path::PathBuf {
        std::path::PathBuf::from(DEFAULT_STORAGE_PATH)
    }

    fn needs_storage_permission(&self) -> bool {
        true
    }

    fn request_all_files_permission(&self, app: &AppHandle) -> Result<bool, String> {
        let status = self.check_permission_status();
        if status.needs_user_action {
            open_storage_permission_settings(app);
            info!("Requested READ_MEDIA_IMAGES permission");
            Ok(false) // User must grant via system dialog
        } else {
            Ok(true)
        }
    }

    // ========== 窗口与UI相关 ==========

    fn hide_main_window(&self, _app: &AppHandle) -> Result<(), String> {
        // Android 没有"窗口"概念，直接返回成功
        Ok(())
    }

    fn select_save_directory(&self, _app: &AppHandle) -> Result<Option<String>, String> {
        // Android 使用固定路径，直接返回默认路径
        Ok(Some(DEFAULT_STORAGE_PATH.to_string()))
    }

    fn open_all_files_access_settings(&self, app: &AppHandle) -> Result<(), String> {
        open_storage_permission_settings(app);
        Ok(())
    }
}
