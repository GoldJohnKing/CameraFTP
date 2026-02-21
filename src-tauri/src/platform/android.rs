use tauri::AppHandle;
use tauri::Manager;

/// Android foreground service wrapper
/// 在 Android 上实现后台 FTP 服务器运行

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
