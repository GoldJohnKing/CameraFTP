use tauri::{command, AppHandle, Manager, Runtime};
use tracing::info;

/// SAF选择器结果
#[derive(Debug, Clone, serde::Serialize)]
pub struct SAFPickerResult {
    pub uri: Option<String>,
    pub name: Option<String>,
}

/// 打开SAF目录选择器（Android）或系统对话框（桌面）
#[command]
pub async fn open_saf_picker<R: Runtime>(
    app: AppHandle<R>,
    initial_uri: Option<String>,
) -> Result<SAFPickerResult, String> {
    #[cfg(target_os = "android")]
    {
        // Android: 使用Tauri插件调用Kotlin代码
        // 插件会自动处理Activity Result
        info!("Opening SAF picker on Android with initial_uri: {:?}", initial_uri);
        
        // 调用Android插件
        let result: Result<serde_json::Value, tauri::plugin::PluginInvokeError> = 
            app.invoke(
                "plugin:sa-fpicker|openSAFPicker",
                serde_json::json!({
                    "initialUri": initial_uri
                }),
            ).await;
        
        match result {
            Ok(value) => {
                let uri = value.get("uri").and_then(|v| {
                    if v.is_null() { None } else { v.as_str().map(|s| s.to_string()) }
                });
                let name = value.get("name").and_then(|v| {
                    if v.is_null() { None } else { v.as_str().map(|s| s.to_string()) }
                });
                
                Ok(SAFPickerResult { uri, name })
            }
            Err(e) => {
                Err(format!("Plugin error: {}", e))
            }
        }
    }
    
    #[cfg(not(target_os = "android"))]
    {
        // 桌面端: 使用对话框
        info!("Opening directory picker on desktop");
        
        use tauri_plugin_dialog::DialogExt;
        
        let folder = app.dialog()
            .file()
            .set_title("选择存储路径")
            .blocking_pick_folder();
        
        if let Some(path) = folder {
            let path_str = path.to_string_lossy().to_string();
            let name = path.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "Selected Folder".to_string());
            
            Ok(SAFPickerResult {
                uri: Some(path_str),
                name: Some(name),
            })
        } else {
            Ok(SAFPickerResult { uri: None, name: None })
        }
    }
}

/// 兼容旧接口：请求打开SAF选择器
#[command]
pub fn request_saf_picker(app: AppHandle, initial_uri: Option<String>) {
    // 为了保持兼容性，仍然发送事件
    // 但主要逻辑应该使用 open_saf_picker
    #[cfg(target_os = "android")]
    {
        let _ = app.emit("android-open-saf-picker", serde_json::json!({
            "initial_uri": initial_uri,
        }));
        info!("Emitted android-open-saf-picker event (legacy)");
    }
}

/// 接收SAF选择器结果
#[command]
pub fn on_saf_picker_result(app: AppHandle, uri: Option<String>) {
    let _ = app.emit("saf-picker-result", serde_json::json!({
        "uri": uri,
    }));
    info!(uri = ?uri, "Received SAF picker result");
}

/// 桌面端实现
#[cfg(not(target_os = "android"))]
async fn select_save_directory_impl<R: Runtime>(
    app: AppHandle<R>,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    
    let folder_path = tokio::task::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("选择存储路径")
            .blocking_pick_folder()
    })
    .await
    .map_err(|e| format!("Task failed: {}", e))?;
    
    Ok(folder_path.and_then(|p| p.as_path().map(|path| path.to_string_lossy().to_string())))
}
