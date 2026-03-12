// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use tauri::{command, AppHandle, Manager, State};
use tracing::instrument;

use crate::auto_open::AutoOpenService;
use crate::config::{AppConfig, PreviewWindowConfig};
use crate::crypto;
use crate::error::AppError;
use crate::file_index::FileIndexService;
use std::sync::Arc;

#[command]
#[instrument]
pub fn load_config() -> AppConfig {
    AppConfig::load()
}

#[command]
#[instrument(skip(config, file_index))]
pub async fn save_config(
    config: AppConfig,
    file_index: State<'_, Arc<FileIndexService>>,
) -> Result<(), AppError> {
    // 加载旧配置以比较 save_path
    let old_config = AppConfig::load();
    let old_save_path = old_config.save_path.clone();
    let new_save_path = config.save_path.clone();
    
    // 保存新配置
    config.save()?;
    tracing::info!("Configuration saved successfully");
    
    // 如果 save_path 变化，重新扫描新目录
    if old_save_path != new_save_path {
        tracing::info!("save_path changed from {:?} to {:?}, triggering rescan", old_save_path, new_save_path);
        file_index.update_save_path(new_save_path).await?;
    }
    
    Ok(())
}

/// 保存认证配置（使用 Argon2id 哈希密码）
#[command]
#[instrument]
pub fn save_auth_config(
    anonymous: bool,
    username: String,
    password: String,
) -> Result<(), AppError> {
    use crate::config::AuthConfig;

    let mut config = AppConfig::load();

    let password_hash = if anonymous || password.is_empty() {
        String::new()
    } else {
        crypto::hash_password(password).hash
    };

    config.advanced_connection.auth = AuthConfig {
        anonymous,
        username,
        password_hash,
    };

    config.save()?;

    tracing::info!("Auth config saved with Argon2id hash");
    Ok(())
}

/// 获取固定存储路径（Android）或当前配置路径（桌面）
#[command]
pub fn get_storage_path() -> Result<String, String> {
    crate::platform::get_platform().get_storage_path()
}

/// 验证保存路径是否有效
#[command]
pub fn validate_save_path(path: String) -> bool {
    let path_obj = std::path::PathBuf::from(&path);
    path_obj.exists() && path_obj.is_dir()
}

/// 选择保存目录
#[command]
pub async fn select_save_directory(app: AppHandle) -> Result<Option<String>, String> {
    let platform = crate::platform::get_platform();
    let result = platform.select_save_directory(&app)?;
    
    // 如果平台返回 None（如 Windows），则使用对话框选择
    #[cfg(target_os = "windows")]
    if result.is_none() {
        use tauri_plugin_dialog::DialogExt;

        let folder_path = tokio::task::spawn_blocking(move || {
            app.dialog()
                .file()
                .set_title("选择存储路径")
                .blocking_pick_folder()
        })
        .await
        .map_err(|e| format!("Task failed: {}", e))?;

        return Ok(folder_path.and_then(|p| p.as_path().map(|path| path.to_string_lossy().to_string())));
    }
    
    Ok(result)
}

// ============================================================================
// 自动预览配置命令（Windows）
// ============================================================================

/// 获取预览窗口配置
#[command]
pub async fn get_preview_config(
    auto_open: State<'_, AutoOpenService>,
) -> Result<PreviewWindowConfig, AppError> {
    Ok(auto_open.get_config().await)
}

/// 设置预览窗口配置
#[command]
pub async fn set_preview_config(
    auto_open: State<'_, AutoOpenService>,
    config: PreviewWindowConfig,
) -> Result<(), AppError> {
    auto_open.update_config(config).await;
    Ok(())
}

/// 手动打开预览窗口（遵循用户配置的打开方式）
#[command]
pub async fn open_preview_window(
    app: AppHandle,
    file_path: String,
) -> Result<(), AppError> {
    let path = std::path::PathBuf::from(&file_path);
    
    // 先在 FileIndexService 中查找并设置索引
    let file_index = app.state::<Arc<FileIndexService>>();
    if let Some(index) = file_index.find_file_index(&path).await {
        file_index.navigate_to(index).await?;
    }
    
    // 使用 AutoOpenService 来处理，它会根据配置决定打开方式
    let auto_open = app.state::<AutoOpenService>();
    auto_open.open_image(&path).await
}

/// 选择可执行文件（用于自定义打开程序）
#[command]
#[cfg_attr(target_os = "android", allow(unused_variables))]
pub async fn select_executable_file(app: AppHandle) -> Result<Option<String>, AppError> {
    #[cfg(target_os = "windows")]
    {
        use tauri_plugin_dialog::DialogExt;

        let file_path: Option<tauri_plugin_dialog::FilePath> = tokio::task::spawn_blocking(move || {
            app.dialog()
                .file()
                .set_title("选择程序")
                .add_filter("可执行文件", &["exe"])
                .blocking_pick_file()
        })
        .await
        .map_err(|e| AppError::Other(format!("Task failed: {}", e)))?;

        return Ok(file_path.and_then(|p| p.as_path().map(|path| path.to_string_lossy().to_string())));
    }

    #[cfg(target_os = "android")]
    {
        Ok(None)
    }
}

/// 打开外部链接
#[command]
pub async fn open_external_link(url: String) -> Result<(), AppError> {
    #[cfg(target_os = "windows")]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;
        use windows::core::PCWSTR;
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

        let url_wide: Vec<u16> = OsStr::new(&url)
            .encode_wide()
            .chain(Some(0))
            .collect();
        let open_wide: Vec<u16> = OsStr::new("open")
            .encode_wide()
            .chain(Some(0))
            .collect();

        let result = unsafe {
            ShellExecuteW(
                None,
                PCWSTR::from_raw(open_wide.as_ptr()),
                PCWSTR::from_raw(url_wide.as_ptr()),
                None,
                None,
                SW_SHOWNORMAL,
            )
        };

        // ShellExecuteW returns HINSTANCE, success > 32, failure <= 32
        if result.0 as isize <= 32 {
            return Err(AppError::Other(format!(
                "ShellExecute failed with code {:?}",
                result.0
            )));
        }

        Ok(())
    }

    #[cfg(target_os = "android")]
    {
        let _ = url;
        // Android 平台通过 JavaScript bridge 处理外部链接
        Ok(())
    }
}

/// 打开文件夹并选中文件（Windows 资源管理器）
#[command]
pub async fn open_folder_select_file(file_path: String) -> Result<(), AppError> {
    #[cfg(target_os = "windows")]
    {
        crate::auto_open::windows::open_folder_and_select_file(&std::path::PathBuf::from(&file_path))
    }

    #[cfg(target_os = "android")]
    {
        let _ = file_path;
        Ok(())
    }
}