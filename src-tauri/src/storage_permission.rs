use tauri::AppHandle;

#[cfg(target_os = "android")]
use tracing::info;

#[cfg(target_os = "android")]
use crate::platform::android;

// 重新导出 Android 平台的类型
#[cfg(target_os = "android")]
pub use crate::platform::android::{StorageInfo, PermissionStatus};

// 非 Android 平台的类型定义
#[cfg(not(target_os = "android"))]
#[derive(Debug, Clone, serde::Serialize)]
pub struct StorageInfo {
    pub display_name: String,
    pub path: String,
    pub exists: bool,
    pub writable: bool,
    pub has_all_files_access: bool,
}

#[cfg(not(target_os = "android"))]
#[derive(Debug, Clone, serde::Serialize)]
pub struct PermissionStatus {
    pub has_all_files_access: bool,
    pub needs_user_action: bool,
}

/// 获取固定存储路径信息
#[tauri::command]
pub async fn get_storage_info() -> Result<StorageInfo, String> {
    #[cfg(target_os = "android")]
    {
        Ok(android::get_storage_info())
    }
    
    #[cfg(not(target_os = "android"))]
    {
        Ok(StorageInfo {
            display_name: "本地存储".to_string(),
            path: String::new(),
            exists: false,
            writable: false,
            has_all_files_access: false,
        })
    }
}

/// 检查权限状态
#[tauri::command]
pub async fn check_permission_status() -> Result<PermissionStatus, String> {
    #[cfg(target_os = "android")]
    {
        Ok(android::check_permission_status())
    }
    
    #[cfg(not(target_os = "android"))]
    {
        Ok(PermissionStatus {
            has_all_files_access: true,
            needs_user_action: false,
        })
    }
}

/// 请求"所有文件访问权限"
#[tauri::command]
pub async fn request_all_files_permission(app: AppHandle) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        android::open_manage_storage_settings(&app);
        info!("Requested all files access permission");
        Ok(())
    }
    
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        Ok(())
    }
}

/// 确保存储目录存在且可写
#[tauri::command]
pub async fn ensure_storage_ready() -> Result<String, String> {
    #[cfg(target_os = "android")]
    {
        android::ensure_storage_ready()
    }
    
    #[cfg(not(target_os = "android"))]
    {
        Err("此功能仅在 Android 平台可用".to_string())
    }
}

/// 检查存储权限
#[tauri::command]
pub async fn check_storage_permission() -> Result<bool, String> {
    #[cfg(target_os = "android")]
    {
        Ok(android::check_all_files_permission())
    }
    
    #[cfg(not(target_os = "android"))]
    {
        Ok(true)
    }
}

/// 检查服务器启动前提条件
#[tauri::command]
pub async fn check_server_start_prerequisites() -> Result<ServerStartCheckResult, String> {
    #[cfg(target_os = "android")]
    {
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
    
    #[cfg(not(target_os = "android"))]
    {
        Ok(ServerStartCheckResult {
            can_start: true,
            reason: None,
            storage_info: None,
        })
    }
}

/// 服务器启动检查结果
#[derive(Debug, Clone, serde::Serialize)]
pub struct ServerStartCheckResult {
    pub can_start: bool,
    pub reason: Option<String>,
    pub storage_info: Option<StorageInfo>,
}
