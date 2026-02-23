use tauri::AppHandle;
use tracing::{debug, error, info};

/// 默认存储目录名称
pub const DEFAULT_STORAGE_DIR_NAME: &str = "CameraFTP";

/// 默认存储路径：DCIM/CameraFTP
/// 这是固定路径，用户不能更改
pub const DEFAULT_STORAGE_PATH: &str = "/storage/emulated/0/DCIM/CameraFTP";

/// 显示名称
pub const STORAGE_DISPLAY_NAME: &str = "DCIM/CameraFTP";

/// 存储路径信息
#[derive(Debug, Clone, serde::Serialize)]
pub struct StorageInfo {
    /// 显示名称
    pub display_name: String,
    /// 完整文件系统路径
    pub path: String,
    /// 路径是否存在
    pub exists: bool,
    /// 是否可写
    pub writable: bool,
    /// 是否有所有文件访问权限
    pub has_all_files_access: bool,
}

/// 权限状态
#[derive(Debug, Clone, serde::Serialize)]
pub struct PermissionStatus {
    /// 是否有"所有文件访问权限"
    pub has_all_files_access: bool,
    /// 是否需要用户操作
    pub needs_user_action: bool,
}

/// 获取默认存储路径
pub fn get_default_storage_path() -> String {
    DEFAULT_STORAGE_PATH.to_string()
}

/// 获取存储路径显示名称
pub fn get_storage_display_name() -> String {
    STORAGE_DISPLAY_NAME.to_string()
}

/// 获取存储路径信息
#[cfg(target_os = "android")]
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

#[cfg(not(target_os = "android"))]
pub fn get_storage_info() -> StorageInfo {
    StorageInfo {
        display_name: "本地存储".to_string(),
        path: "./ftp_uploads".to_string(),
        exists: true,
        writable: true,
        has_all_files_access: true,
    }
}

/// 检查权限状态
#[cfg(target_os = "android")]
pub fn check_permission_status() -> PermissionStatus {
    let has_access = check_all_files_permission();
    PermissionStatus {
        has_all_files_access: has_access,
        needs_user_action: !has_access,
    }
}

#[cfg(not(target_os = "android"))]
pub fn check_permission_status() -> PermissionStatus {
    PermissionStatus {
        has_all_files_access: true,
        needs_user_action: false,
    }
}

/// 检查是否有"所有文件访问权限"
/// 通过尝试写入 DCIM 目录来判断
#[cfg(target_os = "android")]
pub fn check_all_files_permission() -> bool {
    can_write_to_dcim()
}

#[cfg(not(target_os = "android"))]
pub fn check_all_files_permission() -> bool {
    true
}

/// 尝试写入 DCIM 目录来检查权限
#[cfg(target_os = "android")]
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
#[cfg(target_os = "android")]
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

#[cfg(not(target_os = "android"))]
pub fn validate_path_writable(_path: &str) -> bool {
    true
}

/// 确保存储目录存在且可写
#[cfg(target_os = "android")]
pub fn ensure_storage_ready() -> Result<String, String> {
    let path = DEFAULT_STORAGE_PATH;
    let path_buf = std::path::PathBuf::from(path);

    // 检查权限
    if !check_all_files_permission() {
        return Err(
            "需要授予\"所有文件访问权限\"才能使用存储功能。请在设置中开启权限。".to_string(),
        );
    }

    // 创建目录（如果不存在）
    if !path_buf.exists() {
        std::fs::create_dir_all(&path_buf).map_err(|e| format!("无法创建存储目录: {}", e))?;
        info!("Created storage directory: {}", path);
    }

    // 验证可写
    if !validate_path_writable(path) {
        return Err("存储目录不可写，请检查权限设置".to_string());
    }

    Ok(path.to_string())
}

#[cfg(not(target_os = "android"))]
pub fn ensure_storage_ready() -> Result<String, String> {
    Ok("./ftp_uploads".to_string())
}

/// 打开"所有文件访问权限"设置页面
pub fn open_manage_storage_settings(app: &AppHandle) {
    #[cfg(target_os = "android")]
    {
        use tauri::Emitter;
        let _ = app.emit("android-open-manage-storage-settings", ());
        info!("Requesting to open manage storage settings");
    }
}

/// 启动前台服务
pub fn start_foreground_service(app: &AppHandle) {
    #[cfg(target_os = "android")]
    {
        use tauri::Emitter;
        let _ = app.emit("android-start-foreground-service", ());
    }
}

/// 停止前台服务
pub fn stop_foreground_service(app: &AppHandle) {
    #[cfg(target_os = "android")]
    {
        use tauri::Emitter;
        let _ = app.emit("android-stop-foreground-service", ());
    }
}

/// 获取 Android 设备信息
#[cfg(target_os = "android")]
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

#[cfg(not(target_os = "android"))]
pub fn get_device_info() -> DeviceInfo {
    DeviceInfo {
        platform: "unknown".to_string(),
        version: "unknown".to_string(),
        model: "unknown".to_string(),
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DeviceInfo {
    pub platform: String,
    pub version: String,
    pub model: String,
}

/// 获取 Android 版本
#[cfg(target_os = "android")]
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
#[cfg(target_os = "android")]
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
    #[cfg(target_os = "android")]
    {
        use tauri::Emitter;
        let _ = app.emit(
            "android-show-notification",
            serde_json::json!({
                "title": title,
                "body": body,
            }),
        );
    }
}
