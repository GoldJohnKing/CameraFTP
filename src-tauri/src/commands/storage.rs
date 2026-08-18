// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use tauri::{command, AppHandle};

use crate::error::AppError;
use crate::platform::{
    get_platform as get_platform_service,
    PermissionStatus,
    ServerStartCheckResult,
    StorageInfo,
};

// ============================================================================
// 存储权限管理命令（从 storage_permission.rs 迁移）
// ============================================================================

/// 获取存储路径信息（Android 为固定路径，Windows 返回空 path 由用户配置）
#[command]
pub async fn get_storage_info() -> Result<StorageInfo, AppError> {
    // 平台实现内部包含阻塞的文件系统检查（exists/create_dir_all/可写探测），
    // 放入 spawn_blocking 避免阻塞异步命令线程
    tokio::task::spawn_blocking(|| get_platform_service().get_storage_info())
        .await
        .map_err(|e| AppError::Other(format!("Storage info task failed: {}", e)))
}

/// 检查权限状态
#[command]
pub async fn check_permission_status() -> Result<PermissionStatus, AppError> {
    Ok(get_platform_service().check_permission_status())
}

/// 确保存储目录存在且可写
#[command]
pub async fn ensure_storage_ready(app: AppHandle) -> Result<String, AppError> {
    get_platform_service()
        .ensure_storage_ready(&app)
        .map_err(AppError::StoragePermissionError)
}

/// 检查服务器启动前提条件
#[command]
pub async fn check_server_start_prerequisites() -> Result<ServerStartCheckResult, AppError> {
    Ok(get_platform_service().check_server_start_prerequisites())
}

/// 获取当前平台名称
#[command]
pub fn get_platform() -> String {
    crate::platform::get_platform().name().to_string()
}

/// 设置开机自启
#[command]
pub fn set_autostart_command(enable: bool) -> Result<(), String> {
    crate::platform::get_platform().set_autostart(enable)
}

/// 获取开机自启状态
#[command]
pub fn get_autostart_status() -> Result<bool, String> {
    crate::platform::get_platform().is_autostart_enabled()
}
