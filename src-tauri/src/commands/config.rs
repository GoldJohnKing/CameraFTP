// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use tauri::{command, AppHandle, Manager, State};
use tracing::instrument;
use serde::Deserialize;

use crate::auto_open::AutoOpenService;
use crate::config::{AppConfig, PreviewWindowConfig};
use crate::config_service::ConfigService;
use crate::crypto;
use crate::error::AppError;
use crate::file_index::FileIndexService;
use std::sync::Arc;

fn load_config_from_service(config_service: &ConfigService) -> AppConfig {
    match config_service.get() {
        Ok(config) => config,
        Err(e) => {
            tracing::error!(error = %e, "Failed to read config from ConfigService, returning defaults");
            AppConfig::default()
        }
    }
}

fn save_auth_config_with_service(
    config_service: &ConfigService,
    anonymous: bool,
    username: String,
    password: String,
) -> Result<(), AppError> {
    use crate::config::AuthConfig;

    let password_hash = if anonymous || password.is_empty() {
        String::new()
    } else {
        crypto::hash_password(password).hash
    };

    config_service.mutate_and_persist(move |config| {
        config.advanced_connection.auth = AuthConfig {
            anonymous,
            username,
            password_hash,
        };
    })?;

    tracing::info!("Auth config saved with Argon2id hash");
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewWindowConfigPatch {
    pub enabled: Option<bool>,
    pub method: Option<crate::config::ImageOpenMethod>,
    pub custom_path: Option<Option<String>>,
    pub auto_bring_to_front: Option<bool>,
}

impl PreviewWindowConfigPatch {
    fn apply_to(self, mut current: PreviewWindowConfig) -> PreviewWindowConfig {
        if let Some(enabled) = self.enabled {
            current.enabled = enabled;
        }
        if let Some(method) = self.method {
            current.method = method;
        }
        if let Some(custom_path) = self.custom_path {
            current.custom_path = custom_path;
        }
        if let Some(auto_bring_to_front) = self.auto_bring_to_front {
            current.auto_bring_to_front = auto_bring_to_front;
        }
        current
    }
}

fn merge_backend_owned_fields(mut incoming: AppConfig, current: &AppConfig) -> AppConfig {
    incoming.preview_config = current.preview_config.clone();
    incoming
}

fn update_preview_config_with_service(
    config_service: &ConfigService,
    patch: PreviewWindowConfigPatch,
) -> Result<PreviewWindowConfig, AppError> {
    config_service.mutate_and_persist(move |app_config| {
        let current = app_config.preview_config.clone().unwrap_or_default();
        let merged = patch.apply_to(current);
        app_config.preview_config = Some(merged.clone());
        merged
    })
}

#[command]
#[instrument(skip(config_service))]
pub fn load_config(config_service: State<'_, Arc<ConfigService>>) -> AppConfig {
    load_config_from_service(config_service.inner().as_ref())
}

#[command]
#[instrument(skip(config, config_service, file_index))]
pub async fn save_config(
    config: AppConfig,
    config_service: State<'_, Arc<ConfigService>>,
    file_index: State<'_, Arc<FileIndexService>>,
) -> Result<(), AppError> {
    let old_save_path = config_service.mutate_and_persist(move |current| {
        let old_save_path = current.save_path.clone();
        *current = merge_backend_owned_fields(config, current);
        old_save_path
    })?;
    let new_save_path = config_service.get()?.save_path;

    tracing::info!("Configuration saved successfully");

    if old_save_path != new_save_path {
        tracing::info!("save_path changed from {:?} to {:?}, triggering rescan", old_save_path, new_save_path);
        file_index.update_save_path(new_save_path).await?;
    }

    Ok(())
}

/// 保存认证配置（使用 Argon2id 哈希密码）
#[command]
#[instrument(skip(config_service, password))]
pub fn save_auth_config(
    config_service: State<'_, Arc<ConfigService>>,
    anonymous: bool,
    username: String,
    password: String,
) -> Result<(), AppError> {
    save_auth_config_with_service(config_service.inner().as_ref(), anonymous, username, password)
}

/// 获取固定存储路径（Android）或当前配置路径（桌面）
#[command]
pub fn get_storage_path(app: AppHandle) -> Result<String, String> {
    crate::platform::get_platform().get_storage_path(&app)
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

#[command]
pub async fn update_preview_config(
    auto_open: State<'_, AutoOpenService>,
    config_service: State<'_, Arc<ConfigService>>,
    patch: PreviewWindowConfigPatch,
) -> Result<PreviewWindowConfig, AppError> {
    let persisted = update_preview_config_with_service(config_service.inner().as_ref(), patch)?;
    auto_open.broadcast_config_changed(persisted.clone()).await;
    Ok(persisted)
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

#[cfg(test)]
mod tests {
    use crate::config::{ImageOpenMethod, PreviewWindowConfig};
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn helper_load_uses_service_state() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let config_path = temp_dir.path().join("config.json");
        let service = ConfigService::new_with_path(config_path);
        service.load().expect("failed to load config");

        let mut updated = service.get().expect("failed to get config");
        updated.port = 3777;
        service.update(updated).expect("failed to update config");

        let loaded = load_config_from_service(&service);
        assert_eq!(loaded.port, 3777);
    }

    #[test]
    fn helper_save_auth_persists_via_service() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let config_path = temp_dir.path().join("config.json");
        let service = ConfigService::new_with_path(config_path.clone());
        service.load().expect("failed to load config");

        save_auth_config_with_service(
            &service,
            false,
            "camera-user".to_string(),
            "secret-pass".to_string(),
        )
        .expect("failed to save auth config");

        let persisted_service = ConfigService::new_with_path(config_path);
        let reloaded = persisted_service.load().expect("failed to reload config");
        let auth = reloaded.advanced_connection.auth;

        assert!(!auth.anonymous);
        assert_eq!(auth.username, "camera-user");
        assert!(!auth.password_hash.is_empty());
        assert_ne!(auth.password_hash, "secret-pass");
    }

    #[test]
    fn helper_merge_backend_owned_fields_preserves_preview_config() {
        let current = AppConfig {
            preview_config: Some(PreviewWindowConfig {
                enabled: true,
                method: ImageOpenMethod::WindowsPhotos,
                custom_path: Some("C:/Program Files/Photos/photos.exe".to_string()),
                auto_bring_to_front: true,
            }),
            ..AppConfig::default()
        };
        let incoming = AppConfig {
            preview_config: Some(PreviewWindowConfig::default()),
            ..AppConfig::default()
        };

        let merged = merge_backend_owned_fields(incoming, &current);
        let preview = merged
            .preview_config
            .expect("preview should still be present");
        assert!(matches!(preview.method, ImageOpenMethod::WindowsPhotos));
        assert!(preview.auto_bring_to_front);
    }

    #[test]
    fn helper_preview_patch_updates_only_requested_fields() {
        let current = PreviewWindowConfig {
            enabled: true,
            method: ImageOpenMethod::BuiltInPreview,
            custom_path: Some("viewer.exe".to_string()),
            auto_bring_to_front: false,
        };
        let patch = PreviewWindowConfigPatch {
            enabled: None,
            method: Some(ImageOpenMethod::SystemDefault),
            custom_path: None,
            auto_bring_to_front: Some(true),
        };

        let merged = patch.apply_to(current);
        assert!(matches!(merged.method, ImageOpenMethod::SystemDefault));
        assert_eq!(merged.custom_path, Some("viewer.exe".to_string()));
        assert!(merged.auto_bring_to_front);
    }

    #[test]
    fn helper_update_preview_patch_merges_against_latest_persisted_config() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let config_path = temp_dir.path().join("config.json");
        let service = ConfigService::new_with_path(config_path.clone());
        service.load().expect("failed to load config");

        let updated = update_preview_config_with_service(
            &service,
            PreviewWindowConfigPatch {
                enabled: Some(false),
                method: Some(ImageOpenMethod::WindowsPhotos),
                custom_path: Some(Some("C:/Program Files/WindowsApps/photos.exe".to_string())),
                auto_bring_to_front: Some(false),
            },
        )
        .expect("failed to initialize preview config");

        assert!(!updated.enabled);
        assert!(!updated.auto_bring_to_front);

        let updated = update_preview_config_with_service(
            &service,
            PreviewWindowConfigPatch {
                enabled: Some(true),
                method: None,
                custom_path: None,
                auto_bring_to_front: Some(true),
            },
        )
        .expect("failed to update preview config");

        assert!(updated.enabled);
        assert!(updated.auto_bring_to_front);
        assert!(matches!(updated.method, ImageOpenMethod::WindowsPhotos));
        assert_eq!(
            updated.custom_path,
            Some("C:/Program Files/WindowsApps/photos.exe".to_string())
        );

        let persisted = service
            .get()
            .expect("failed to read persisted config")
            .preview_config
            .expect("preview config should exist");
        assert!(persisted.enabled);
        assert!(matches!(persisted.method, ImageOpenMethod::WindowsPhotos));
    }

    #[test]
    fn helper_update_preview_patch_returns_error_when_persistence_fails() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let blocked_parent = temp_dir.path().join("blocked-parent");
        std::fs::write(&blocked_parent, "not a directory").expect("failed to create blocker file");

        let service = ConfigService::new_with_path(blocked_parent.join("config.json"));
        let result = update_preview_config_with_service(
            &service,
            PreviewWindowConfigPatch {
                enabled: Some(true),
                method: None,
                custom_path: None,
                auto_bring_to_front: None,
            },
        );

        assert!(result.is_err());
    }
}
