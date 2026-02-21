use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use tauri::AppHandle;
use tauri::Manager;

/// 目录选择回调类型
type DirectoryPickerCallback = Box<dyn FnOnce(Option<String>) + Send>;

/// 存储待执行的回调
static DIRECTORY_PICKER_CALLBACK: Mutex<Option<DirectoryPickerCallback>> = Mutex::new(None);

/// Android foreground service wrapper
/// 在 Android 上实现后台 FTP 服务器运行

/// 请求 Android SAF 目录选择器
/// 返回选择的目录 URI (content://...)
#[cfg(target_os = "android")]
pub fn request_directory_picker<F>(app: &AppHandle, callback: F)
where
    F: FnOnce(Option<String>) + Send + 'static,
{
    use tauri::Emitter;

    // 存储回调
    if let Ok(mut cb) = DIRECTORY_PICKER_CALLBACK.lock() {
        *cb = Some(Box::new(callback));
    }

    // 发送事件给前端，前端调用 Android 的 SAF 选择器
    let _ = app.emit("android-request-directory-picker", ());
}

/// Android 选择器返回结果时调用
#[cfg(target_os = "android")]
pub fn on_directory_selected(uri: Option<String>) {
    if let Ok(mut cb) = DIRECTORY_PICKER_CALLBACK.lock() {
        if let Some(callback) = cb.take() {
            callback(uri);
        }
    }
}

#[cfg(not(target_os = "android"))]
pub fn request_directory_picker<F>(_app: &AppHandle, _callback: F)
where
    F: FnOnce(Option<String>) + Send + 'static,
{
    // 非 Android 平台直接返回 None
    _callback(None);
}

/// 获取持久化的存储目录 URI（从 Android SharedPreferences）
#[cfg(target_os = "android")]
pub fn get_persisted_directory_uri(_app: &AppHandle) -> Option<String> {
    // 注意：实际实现需要通过 JNI 读取 SharedPreferences
    // 这里返回 None，由前端通过 JS 桥获取后传回
    None
}

#[cfg(not(target_os = "android"))]
pub fn get_persisted_directory_uri(_app: &AppHandle) -> Option<String> {
    None
}

/// 获取推荐存储路径
#[cfg(target_os = "android")]
pub fn get_recommended_storage_path(_app: &AppHandle) -> String {
    // 优先级：
    // 1. 持久化的 SAF URI
    // 2. /DCIM/CameraFTPCompanion
    // 3. /Pictures/CameraFTPCompanion
    // 4. 应用私有目录

    // 注意：实际路径由前端通过 JS 获取后传回
    // 这里返回空字符串表示使用默认逻辑
    String::new()
}

#[cfg(not(target_os = "android"))]
pub fn get_recommended_storage_path(_app: &AppHandle) -> String {
    String::new()
}

/// 检查 SAF 权限是否有效
#[cfg(target_os = "android")]
pub fn check_saf_permission(_app: &AppHandle, _uri: &str) -> bool {
    // 前端通过 JS 验证后传回结果
    true
}

#[cfg(not(target_os = "android"))]
pub fn check_saf_permission(_app: &AppHandle, _uri: &str) -> bool {
    false
}

/// 启动前台服务
/// 这会让应用在后台继续运行 FTP 服务器
pub fn start_foreground_service(app: &AppHandle) {
    #[cfg(target_os = "android")]
    {
        use tauri::Emitter;
        // 发送事件给前端，由前端调用 Android 原生插件
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

/// 检查是否有后台运行权限
pub fn has_background_permission(_app: &AppHandle) -> bool {
    // Android 13+ 需要特殊权限
    // 实际检查需要在前端通过 Capacitor/Cordova 插件完成
    true
}

/// 请求后台运行权限
pub fn request_background_permission(app: &AppHandle) {
    #[cfg(target_os = "android")]
    {
        use tauri::Emitter;
        let _ = app.emit("android-request-background-permission", ());
    }
}

/// 获取 Android 设备信息
pub fn get_device_info() -> DeviceInfo {
    DeviceInfo {
        platform: "android".to_string(),
        // 实际版本号需要从 Android 系统获取
        version: "14".to_string(),
        model: "Unknown".to_string(),
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DeviceInfo {
    pub platform: String,
    pub version: String,
    pub model: String,
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
