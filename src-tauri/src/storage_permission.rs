use tauri::AppHandle;
use tracing::info;

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
    let status = platform.check_permission_status();

    if status.needs_user_action {
        // 打开设置页面让用户授权
        #[cfg(target_os = "android")]
        {
            crate::platform::android::open_manage_storage_settings(&app);
            info!("Requested all files access permission");
        }

        #[cfg(not(target_os = "android"))]
        {
            let _ = app;
            return Err(AppError::PlatformNotSupported(
                "此功能仅在 Android 平台可用".to_string(),
            ));
        }
    }
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
    let platform = crate::platform::get_platform();
    let storage_info = platform.get_storage_info();
    let permission_status = platform.check_permission_status();

    let can_start =
        storage_info.writable || (permission_status.has_all_files_access && !storage_info.exists);

    let reason = if !can_start {
        if !storage_info.has_all_files_access {
            Some(
                "需要授予\"所有文件访问权限\"才能启动服务器。请在设置中开启权限。".to_string(),
            )
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
