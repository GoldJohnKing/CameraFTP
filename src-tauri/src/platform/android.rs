use super::traits::PlatformService;
use super::types::{PermissionStatus, StorageInfo};
use tauri::{AppHandle, Emitter};
use tracing::{debug, error, info};

/// 默认存储目录名称
pub const DEFAULT_STORAGE_DIR_NAME: &str = "CameraFTP";

/// 默认存储路径：DCIM/CameraFTP
/// 这是固定路径，用户不能更改
pub const DEFAULT_STORAGE_PATH: &str = "/storage/emulated/0/DCIM/CameraFTP";

/// 显示名称
pub const STORAGE_DISPLAY_NAME: &str = "DCIM/CameraFTP";

/// 获取默认存储路径
pub fn get_default_storage_path() -> String {
    DEFAULT_STORAGE_PATH.to_string()
}

/// 获取存储路径显示名称
pub fn get_storage_display_name() -> String {
    STORAGE_DISPLAY_NAME.to_string()
}

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
    let has_access = check_all_files_permission();
    PermissionStatus {
        has_all_files_access: has_access,
        needs_user_action: !has_access,
    }
}

/// 检查是否有"所有文件访问权限"
/// 通过尝试写入 DCIM 目录来判断
pub fn check_all_files_permission() -> bool {
    can_write_to_dcim()
}

/// 尝试写入 DCIM 目录来检查权限
fn can_write_to_dcim() -> bool {
    let dcim_path = "/storage/emulated/0/DCIM";
    let test_file = format!("{}/.permission_test_{}", dcim_path, std::process::id());

    // 尝试创建测试文件
    match std::fs::File::create(&test_file) {
        Ok(_) => {
            let _ = std::fs::remove_file(&test_file);
            debug!("All files access permission: granted (DCIM writable)");
            true
        }
        Err(e) => {
            debug!("All files access permission: denied ({})", e);
            false
        }
    }
}

/// 验证路径是否可写
pub fn validate_path_writable(path: &str) -> bool {
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

    // 尝试写入测试文件
    let test_file = path_buf.join(".ftp_write_test");
    match std::fs::File::create(&test_file) {
        Ok(_) => {
            let _ = std::fs::remove_file(&test_file);
            debug!("Path is writable: {:?}", path_buf);
            true
        }
        Err(e) => {
            error!("Path is not writable: {:?}, error: {}", path_buf, e);
            false
        }
    }
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

/// 打开"所有文件访问权限"设置页面
pub fn open_manage_storage_settings(app: &AppHandle) {
    use tauri::Emitter;
    let _ = app.emit("android-open-manage-storage-settings", ());
    info!("Requesting to open manage storage settings");
}

/// 设备信息
#[derive(Debug, Clone, serde::Serialize)]
pub struct DeviceInfo {
    pub platform: String,
    pub version: String,
    pub model: String,
}

/// 获取 Android 设备信息
pub fn get_device_info() -> DeviceInfo {
    // 尝试从系统属性获取设备信息
    let version = get_android_version();
    let model = get_device_model();

    DeviceInfo {
        platform: "android".to_string(),
        version,
        model,
    }
}

/// 获取 Android 版本
fn get_android_version() -> String {
    std::fs::read_to_string("/system/build.prop")
        .ok()
        .and_then(|content| {
            content
                .lines()
                .find(|line| line.starts_with("ro.build.version.release="))
                .map(|line| line.split('=').nth(1).unwrap_or("unknown").to_string())
        })
        .unwrap_or_else(|| "unknown".to_string())
}

/// 获取设备型号
fn get_device_model() -> String {
    std::fs::read_to_string("/system/build.prop")
        .ok()
        .and_then(|content| {
            content
                .lines()
                .find(|line| line.starts_with("ro.product.model="))
                .map(|line| line.split('=').nth(1).unwrap_or("unknown").to_string())
        })
        .unwrap_or_else(|| "unknown".to_string())
}

/// 显示本地通知
pub fn show_notification(app: &AppHandle, title: &str, body: &str) {
    use tauri::Emitter;
    let _ = app.emit(
        "android-show-notification",
        serde_json::json!({
            "title": title,
            "body": body,
        }),
    );
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
        // 前端通过 PermissionDialog 处理权限检查，这里直接返回可启动
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
            open_manage_storage_settings(app);
            info!("Requested all files access permission");
            Ok(false)
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

    fn get_log_directory(&self) -> std::path::PathBuf {
        std::path::PathBuf::from("/storage/emulated/0/DCIM/CameraFTP/logs")
    }

    fn open_all_files_access_settings(&self, app: &AppHandle) -> Result<(), String> {
        open_manage_storage_settings(app);
        Ok(())
    }
}
