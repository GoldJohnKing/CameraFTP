// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use serde::{Deserialize, Serialize};
#[cfg(target_os = "android")]
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;
use tracing::info;
use tracing::warn;
use ts_rs::TS;

use crate::constants::{DEFAULT_FTP_PORT_ANDROID, DEFAULT_FTP_PORT_WINDOWS};

/// 认证配置
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", default)]
pub struct AuthConfig {
    /// 是否启用匿名访问
    pub anonymous: bool,
    /// 自定义用户名（匿名关闭时使用）
    pub username: String,
    /// 密码哈希（Argon2id，PHC格式已包含盐值）
    pub password_hash: String,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            anonymous: true,
            username: String::new(),
            password_hash: String::new(),
        }
    }
}

/// 高级连接配置
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", default)]
pub struct AdvancedConnectionConfig {
    /// 是否启用高级连接配置
    pub enabled: bool,
    /// 认证配置
    pub auth: AuthConfig,
}

impl Default for AdvancedConnectionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            auth: AuthConfig::default(),
        }
    }
}

/// 图片打开方式枚举
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "kebab-case")]
pub enum ImageOpenMethod {
    BuiltInPreview,
    SystemDefault,
    WindowsPhotos,
    Custom,
}

impl Default for ImageOpenMethod {
    fn default() -> Self {
        ImageOpenMethod::BuiltInPreview
    }
}

/// 预览窗口配置
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct PreviewWindowConfig {
    pub enabled: bool,
    pub method: ImageOpenMethod,
    /// 自定义程序路径（仅当 method 为 Custom 时有效）
    pub custom_path: Option<String>,
    pub auto_bring_to_front: bool,
}

impl Default for PreviewWindowConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            method: ImageOpenMethod::BuiltInPreview,
            custom_path: None,
            auto_bring_to_front: false,
        }
    }
}

/// Android 图片打开方式
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "kebab-case")]
pub enum AndroidImageOpenMethod {
    BuiltInViewer,
    ExternalApp,
}

impl Default for AndroidImageOpenMethod {
    fn default() -> Self {
        AndroidImageOpenMethod::BuiltInViewer
    }
}

/// Android 图片查看器配置
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct AndroidImageViewerConfig {
    pub open_method: AndroidImageOpenMethod,
    #[serde(default)]
    pub auto_open_latest_when_visible: bool,
}

impl Default for AndroidImageViewerConfig {
    fn default() -> Self {
        Self {
            open_method: AndroidImageOpenMethod::default(),
            auto_open_latest_when_visible: false,
        }
    }
}

/// 自动调色配置
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", default)]
pub struct AutoColorGradingConfig {
    /// 是否启用自动调色
    pub enabled: bool,
    /// 调色预设 ID
    #[serde(alias = "presetLutId")]
    pub preset_id: String,
    /// 是否使用自动曝光
    pub use_auto_exposure: bool,
    /// 自动曝光测光模式（仅自动曝光时有效）
    pub metering_mode: String,
    /// 手动曝光补偿值（EV），仅当 use_auto_exposure=false 时有效
    pub manual_ev: f32,
}

impl Default for AutoColorGradingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            preset_id: crate::color_grading::presets::DEFAULT_PRESET_ID.to_string(),
            use_auto_exposure: true,
            metering_mode: "highlight-safe".to_string(),
            manual_ev: 0.0,
        }
    }
}

/// 调色对话框上次使用的参数
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", default)]
pub struct ColorGradingLastUsed {
    /// 调色预设 ID
    pub preset_id: String,
    /// 是否使用自动曝光
    pub use_auto_exposure: bool,
    /// 自动曝光测光模式
    pub metering_mode: String,
    /// 手动曝光补偿值（EV）
    pub manual_ev: f32,
}

impl Default for ColorGradingLastUsed {
    fn default() -> Self {
        Self {
            preset_id: crate::color_grading::presets::DEFAULT_PRESET_ID.to_string(),
            use_auto_exposure: true,
            metering_mode: "highlight-safe".to_string(),
            manual_ev: 0.0,
        }
    }
}

/// 应用数据目录（在应用初始化时设置，使用 OnceLock 实现高效缓存）
static APP_CONFIG_DIR: OnceLock<PathBuf> = OnceLock::new();

/// 设置应用数据目录（在应用初始化时调用）
pub fn set_app_config_dir(dir: PathBuf) {
    match APP_CONFIG_DIR.set(dir) {
        Ok(()) => {
            info!("App config dir set: {:?}", APP_CONFIG_DIR.get().unwrap());
        }
        Err(_) => {
            warn!("App config dir already set, ignoring duplicate initialization");
        }
    }
}

/// 获取应用数据目录（从缓存读取，无需加锁）
fn get_app_config_dir() -> PathBuf {
    APP_CONFIG_DIR.get().cloned().unwrap_or_else(|| {
        #[cfg(target_os = "android")]
        {
            PathBuf::from("/sdcard/Android/data/com.gjk.cameraftpcompanion/files")
        }
        #[cfg(target_os = "windows")]
        {
            dirs::config_dir()
                .map(|d| d.join("cameraftp"))
                .unwrap_or_else(|| PathBuf::from("./config"))
        }
    })
}

/// 获取应用数据目录的公共接口
pub fn app_config_dir() -> PathBuf {
    get_app_config_dir()
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    /// 存储路径（桌面端可自定义，Android 端固定为 DCIM/CameraFTP）
    pub save_path: PathBuf,
    /// FTP 端口
    pub port: u16,
    /// 自动选择端口
    pub auto_select_port: bool,
    /// 高级连接配置
    pub advanced_connection: AdvancedConnectionConfig,
    /// 预览窗口配置（仅 Windows 有效，其他平台为 None）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview_config: Option<PreviewWindowConfig>,
    /// Android 图片查看器配置（仅 Android 有效，其他平台为 None）
    #[serde(
        skip_serializing_if = "Option::is_none",
        default = "default_android_image_viewer"
    )]
    pub android_image_viewer: Option<AndroidImageViewerConfig>,
    /// AI修图配置
    #[serde(default)]
    pub ai_edit: crate::ai_edit::config::AiEditConfig,
    /// 自动调色配置
    #[serde(
        skip_serializing_if = "Option::is_none",
        default = "default_auto_color_grading",
        alias = "autoLut"
    )]
    pub auto_color_grading: Option<AutoColorGradingConfig>,
    /// 调色对话框上次使用的参数
    #[serde(
        skip_serializing_if = "Option::is_none",
        default = "default_color_grading_last_used"
    )]
    pub color_grading_last_used: Option<ColorGradingLastUsed>,
}

#[cfg(target_os = "android")]
fn default_android_image_viewer() -> Option<AndroidImageViewerConfig> {
    Some(AndroidImageViewerConfig::default())
}

#[cfg(not(target_os = "android"))]
const fn default_android_image_viewer() -> Option<AndroidImageViewerConfig> {
    None
}

fn default_auto_color_grading() -> Option<AutoColorGradingConfig> {
    Some(AutoColorGradingConfig::default())
}

fn default_color_grading_last_used() -> Option<ColorGradingLastUsed> {
    Some(ColorGradingLastUsed::default())
}

impl Default for AppConfig {
    fn default() -> Self {
        // Windows 默认使用端口 21，Android 默认使用端口 2121
        let default_port = if cfg!(target_os = "windows") {
            DEFAULT_FTP_PORT_WINDOWS
        } else {
            DEFAULT_FTP_PORT_ANDROID
        };

        // preview_config 仅在 Windows 上启用
        let preview_config = if cfg!(target_os = "windows") {
            Some(PreviewWindowConfig::default())
        } else {
            None
        };

        // android_image_viewer 仅在 Android 上启用
        let android_image_viewer = if cfg!(target_os = "android") {
            Some(AndroidImageViewerConfig::default())
        } else {
            None
        };

        let auto_color_grading = Some(AutoColorGradingConfig::default());
        let color_grading_last_used = Some(ColorGradingLastUsed::default());

        Self {
            save_path: Self::default_pictures_dir(),
            port: default_port,
            auto_select_port: true,
            advanced_connection: AdvancedConnectionConfig::default(),
            preview_config,
            android_image_viewer,
            ai_edit: crate::ai_edit::config::AiEditConfig::default(),
            auto_color_grading,
            color_grading_last_used,
        }
    }
}

impl AppConfig {
    /// 获取默认图片目录
    /// 使用条件编译直接确定平台特定的默认存储路径
    fn default_pictures_dir() -> PathBuf {
        #[cfg(target_os = "android")]
        {
            // 从 constants 模块导入，避免与 platform 模块的循环依赖
            PathBuf::from(crate::constants::ANDROID_DEFAULT_STORAGE_PATH)
        }
        #[cfg(target_os = "windows")]
        {
            dirs::picture_dir().unwrap_or_else(|| PathBuf::from("./pictures"))
        }
    }

    pub fn config_path() -> PathBuf {
        get_app_config_dir().join("config.json")
    }

    pub fn normalized_for_current_platform(self) -> Self {
        #[cfg(target_os = "android")]
        {
            let mut config = self;
            config.save_path = PathBuf::from(crate::constants::ANDROID_DEFAULT_STORAGE_PATH);
            config
        }

        #[cfg(not(target_os = "android"))]
        {
            self
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.port == 0 {
            return Err("Port cannot be 0".to_string());
        }

        if self.save_path.as_os_str().is_empty() {
            return Err("Save path cannot be empty".to_string());
        }

        Ok(())
    }
}

/// 初始化应用数据目录（在应用启动时调用）
pub fn init_app_paths(app_handle: &tauri::AppHandle) {
    use tauri::Manager;

    #[cfg(target_os = "android")]
    let config_dir = app_handle
        .path()
        .data_dir()
        .unwrap_or_else(|_| PathBuf::from("/data/data/com.gjk.cameraftpcompanion/files"));

    #[cfg(target_os = "windows")]
    let config_dir = app_handle
        .path()
        .app_data_dir()
        .expect("Failed to resolve app data dir");

    // 确保配置目录存在
    if let Err(e) = std::fs::create_dir_all(&config_dir) {
        warn!("Failed to create config directory {:?}: {}", config_dir, e);
    }

    set_app_config_dir(config_dir.clone());
    info!("App config dir initialized: {:?}", config_dir);

    // Android: 创建默认存储路径
    #[cfg(target_os = "android")]
    {
        let save_path = crate::platform::get_platform().get_default_storage_path();
        if !save_path.exists() {
            match fs::create_dir_all(&save_path) {
                Ok(_) => info!("Created storage directory: {:?}", save_path),
                Err(e) => warn!(
                    "Could not create storage directory (permission may be required): {}",
                    e
                ),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{AndroidImageViewerConfig, AppConfig};

    #[test]
    fn accepts_legacy_android_image_viewer_without_auto_open_flag() {
        let legacy_payload = r#"{
            "openMethod": "external-app"
        }"#;

        let result = serde_json::from_str::<AndroidImageViewerConfig>(legacy_payload);

        assert!(result.is_ok(), "legacy payload should be accepted");
        let config = result.expect("legacy payload should deserialize");
        assert_eq!(
            config.open_method,
            super::AndroidImageOpenMethod::ExternalApp
        );
        assert!(!config.auto_open_latest_when_visible);
    }

    #[test]
    fn valid_default_config_passes_validation() {
        let config = AppConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn empty_save_path_fails_validation() {
        let mut config = AppConfig::default();
        config.save_path = PathBuf::new();
        assert!(config.validate().is_err());
    }

    #[test]
    fn auth_enabled_with_empty_username_passes_validation() {
        // validate() only checks port and save_path — empty username is allowed
        let mut config = AppConfig::default();
        config.advanced_connection.enabled = true;
        config.advanced_connection.auth.anonymous = false;
        config.advanced_connection.auth.username = String::new();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn auth_enabled_with_valid_username_passes_validation() {
        let mut config = AppConfig::default();
        config.advanced_connection.enabled = true;
        config.advanced_connection.auth.anonymous = false;
        config.advanced_connection.auth.username = "admin".to_string();
        config.advanced_connection.auth.password_hash = "somehash".to_string();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn normalized_for_current_platform_applies_platform_save_path_rules() {
        let mut config = AppConfig::default();
        config.save_path = PathBuf::from("/tmp/custom-cameraftp");

        let normalized = config.normalized_for_current_platform();

        #[cfg(target_os = "android")]
        assert_eq!(
            normalized.save_path,
            PathBuf::from(crate::constants::ANDROID_DEFAULT_STORAGE_PATH)
        );

        #[cfg(not(target_os = "android"))]
        assert_eq!(normalized.save_path, PathBuf::from("/tmp/custom-cameraftp"));
    }
}
