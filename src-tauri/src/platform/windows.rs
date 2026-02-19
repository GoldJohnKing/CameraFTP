use tauri::AppHandle;

pub fn setup_tray(_app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    // 托盘功能需要启用 tray-icon feature
    // 当前暂时禁用，后续在 Cargo.toml 中添加 feature 后可启用
    tracing::info!("System tray feature not enabled, skipping tray setup");
    Ok(())
}
