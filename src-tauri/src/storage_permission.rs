//! 存储权限管理命令
//!
//! 这些命令是 PlatformService trait 方法的 Tauri IPC 包装层。
//! 所有实际逻辑都在 platform 模块中实现。

use tauri::AppHandle;

use crate::error::AppError;

// 统一导出平台类型
pub use crate::platform::{PermissionStatus, ServerStartCheckResult, StorageInfo};

/// 获取固定存储路径信息
#[tauri::command]
pub async fn get_storage_info() -> Result<StorageInfo, AppError> {
    Ok(crate::platform::get_platform().get_storage_info())
}

/// 检查权限状态
#[tauri::command]
pub async fn check_permission_status() -> Result<PermissionStatus, AppError> {
    Ok(crate::platform::get_platform().check_permission_status())
}

/// 请求"所有文件访问权限"
#[tauri::command]
pub async fn request_all_files_permission(app: AppHandle) -> Result<(), AppError> {
    let platform = crate::platform::get_platform();
    
    platform
        .request_all_files_permission(&app)
        .map_err(AppError::StoragePermissionError)?;

    // 如果返回 false，说明需要用户去设置页面授权
    // 这里我们不返回错误，让前端决定如何处理
    Ok(())
}

/// 确保存储目录存在且可写
#[tauri::command]
pub async fn ensure_storage_ready() -> Result<String, AppError> {
    crate::platform::get_platform()
        .ensure_storage_ready()
        .map_err(AppError::StoragePermissionError)
}

/// 检查存储权限
#[tauri::command]
pub async fn check_storage_permission() -> Result<bool, AppError> {
    Ok(
        crate::platform::get_platform()
            .check_permission_status()
            .has_all_files_access,
    )
}

/// 检查服务器启动前提条件
#[tauri::command]
pub async fn check_server_start_prerequisites() -> Result<ServerStartCheckResult, AppError> {
    Ok(crate::platform::get_platform().check_server_start_prerequisites())
}

/// 检查是否需要存储权限（用于前端 UI 判断）
#[tauri::command]
pub async fn needs_storage_permission() -> bool {
    crate::platform::get_platform().needs_storage_permission()
}
