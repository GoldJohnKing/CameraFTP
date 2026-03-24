// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
#[cfg(target_os = "android")]
use std::sync::OnceLock;
#[cfg(target_os = "android")]
use tracing::warn;
use tracing::{error, info};
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
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
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
}

impl Default for AndroidImageViewerConfig {
    fn default() -> Self {
        Self {
            open_method: AndroidImageOpenMethod::default(),
        }
    }
}

/// Android 配置路径（在应用初始化时设置，使用 OnceLock 实现高效缓存）
#[cfg(target_os = "android")]
static ANDROID_CONFIG_PATH: OnceLock<PathBuf> = OnceLock::new();

/// 设置 Android 配置路径（在应用初始化时调用）
#[cfg(target_os = "android")]
pub fn set_android_config_path(config_path: PathBuf) {
    if ANDROID_CONFIG_PATH.set(config_path.clone()).is_err() {
        warn!("Android config path already set, ignoring duplicate initialization");
    } else {
        info!("Android config path set: {:?}", config_path);
    }
}

/// 获取 Android 配置路径（从缓存读取，无需加锁）
#[cfg(target_os = "android")]
fn get_android_config_path() -> PathBuf {
    ANDROID_CONFIG_PATH.get().cloned().unwrap_or_else(|| {
        PathBuf::from("/sdcard/Android/data/com.gjk.cameraftpcompanion/files/config.json")
    })
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
}

fn default_android_image_viewer() -> Option<AndroidImageViewerConfig> {
    if cfg!(target_os = "android") {
        Some(AndroidImageViewerConfig::default())
    } else {
        None
    }
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

        Self {
            save_path: Self::default_pictures_dir(),
            port: default_port,
            auto_select_port: true,
            advanced_connection: AdvancedConnectionConfig::default(),
            preview_config,
            android_image_viewer,
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
        #[cfg(target_os = "android")]
        {
            get_android_config_path()
        }
        #[cfg(target_os = "windows")]
        {
            dirs::config_dir()
                .map(|d| d.join("cameraftp"))
                .unwrap_or_else(|| PathBuf::from("./config"))
                .join("config.json")
        }
    }

    pub fn load() -> Self {
        let path = Self::config_path();

        #[allow(unused_mut)]
        let mut config = if path.exists() {
            match fs::read_to_string(&path) {
                Ok(content) => match serde_json::from_str(&content) {
                    Ok(config) => {
                        info!("Config loaded from {:?}", path);
                        config
                    }
                    Err(e) => {
                        error!("Failed to parse config: {}", e);
                        Self::default()
                    }
                },
                Err(e) => {
                    error!("Failed to read config file: {}", e);
                    Self::default()
                }
            }
        } else {
            Self::default()
        };

        // Android 端强制使用固定存储路径，忽略配置文件中的值
        #[cfg(target_os = "android")]
        {
            let fixed_path = PathBuf::from(crate::constants::ANDROID_DEFAULT_STORAGE_PATH);
            if config.save_path != fixed_path {
                info!(
                    "Android: Overriding save_path from {:?} to fixed path {:?}",
                    config.save_path, fixed_path
                );
                config.save_path = fixed_path;
            }
        }

        // 如果是新创建的默认配置，保存到文件
        if !path.exists() {
            if let Err(e) = config.save() {
                error!("Failed to save default config: {}", e);
            }
        }

        config
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::config_path();

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Android 端强制使用固定存储路径进行保存
        #[cfg(target_os = "android")]
        let config_to_save = {
            let mut cloned = self.clone();
            let fixed_path = PathBuf::from(crate::constants::ANDROID_DEFAULT_STORAGE_PATH);
            if cloned.save_path != fixed_path {
                info!(
                    "Android: Setting save_path to fixed path {:?} before saving",
                    fixed_path
                );
                cloned.save_path = fixed_path;
            }
            cloned
        };

        #[cfg(target_os = "windows")]
        let config_to_save = self;

        let content = serde_json::to_string_pretty(&config_to_save)?;
        fs::write(&path, content)?;

        info!("Config saved to {:?}", path);
        Ok(())
    }
}

/// 初始化 Android 路径（在应用启动时调用）
#[cfg(target_os = "android")]
pub fn init_android_paths(app_handle: &tauri::AppHandle) {
    use tauri::Manager;

    // 配置文件存储在应用私有目录
    let config_path = app_handle
        .path()
        .data_dir()
        .unwrap_or_else(|_| PathBuf::from("/data/data/com.gjk.cameraftpcompanion/files"))
        .join("config.json");

    // 确保配置目录存在
    if let Some(parent) = config_path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            warn!("Failed to create config directory {:?}: {}", parent, e);
        }
    }

    set_android_config_path(config_path.clone());
    info!("Android config path initialized: {:?}", config_path);

    // 通过 PlatformService 获取默认存储路径
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

#[cfg(target_os = "windows")]
pub fn init_android_paths(_app_handle: &tauri::AppHandle) {
    // 非 Android 平台无需初始化
}
