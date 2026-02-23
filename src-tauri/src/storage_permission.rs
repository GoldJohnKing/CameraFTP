use tauri::AppHandle;
use tracing::info;

use crate::platform::android;

// Re-export types from android module for convenience
pub use crate::platform::android::{StorageInfo, PermissionStatus};

/// 获取固定存储路径信息
#[tauri::command]
pub async fn get_storage_info() -> Result<StorageInfo, String> {
    Ok(android::get_storage_info())
}

/// 检查权限状态
#[tauri::command]
pub async fn check_permission_status() -> Result<PermissionStatus, String> {
    Ok(android::check_permission_status())
}

/// 请求"所有文件访问权限"
/// 这会触发前端打开系统设置页面
#[tauri::command]
pub async fn request_all_files_permission(app: AppHandle) -> Result<(), String> {
    android::open_manage_storage_settings(&app);
    info!("Requested all files access permission");
    Ok(())
}

/// 确保存储目录存在且可写
#[tauri::command]
pub async fn ensure_storage_ready() -> Result<String, String> {
    android::ensure_storage_ready()
}

/// 检查存储权限（兼容旧接口）
#[tauri::command]
pub async fn check_storage_permission() -> Result<bool, String> {
    Ok(android::check_all_files_permission())
}

/// 检查服务器启动前提条件
#[tauri::command]
pub async fn check_server_start_prerequisites() -> Result<ServerStartCheckResult, String> {
    let storage_info = android::get_storage_info();
    
    let can_start = storage_info.writable || 
        (android::check_all_files_permission() && !storage_info.exists);
    
    let reason = if !can_start {
        if !storage_info.has_all_files_access {
            Some("需要授予\"所有文件访问权限\"才能启动服务器。请在设置中开启权限。".to_string())
        } else {
            Some("存储路径不可写，请检查权限设置".to_string())
        }
    } else {
        None
    };
    
    Ok(ServerStartCheckResult {
        can_start,
        reason,
        storage_info: Some(storage_info),
    })
}

/// 服务器启动检查结果
#[derive(Debug, Clone, serde::Serialize)]
pub struct ServerStartCheckResult {
    pub can_start: bool,
    pub reason: Option<String>,
    pub storage_info: Option<StorageInfo>,
}
