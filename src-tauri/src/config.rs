use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
#[cfg(target_os = "android")]
use std::sync::OnceLock;
#[cfg(target_os = "android")]
use tracing::warn;
use tracing::{error, info};
use ts_rs::TS;

/// Android 配置路径（在应用初始化时设置，使用 OnceLock 实现高效缓存）
#[cfg(target_os = "android")]
static ANDROID_CONFIG_PATH: OnceLock<PathBuf> = OnceLock::new();

/// 设置 Android 配置路径（在应用初始化时调用）
#[cfg(target_os = "android")]
pub fn set_android_config_path(config_path: PathBuf) {
    if let Err(_) = ANDROID_CONFIG_PATH.set(config_path.clone()) {
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
pub struct AppConfig {
    /// 存储路径（桌面端可自定义，Android 端固定为 DCIM/CameraFTP）
    pub save_path: PathBuf,
    /// FTP 端口
    pub port: u16,
    /// 自动选择端口
    pub auto_select_port: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            save_path: Self::default_pictures_dir(),
            port: 2121,
            auto_select_port: true,
        }
    }
}

impl AppConfig {
    /// 获取默认图片目录
    /// 通过 PlatformService trait 获取平台特定的默认存储路径
    fn default_pictures_dir() -> PathBuf {
        crate::platform::get_platform().get_default_storage_path()
    }

    pub fn config_path() -> PathBuf {
        #[cfg(target_os = "android")]
        {
            get_android_config_path()
        }
        #[cfg(not(target_os = "android"))]
        {
            dirs::config_dir()
                .map(|d| d.join("camera-ftp-companion"))
                .unwrap_or_else(|| PathBuf::from("./config"))
                .join("config.json")
        }
    }

    pub fn load() -> Self {
        let path = Self::config_path();

        if path.exists() {
            match fs::read_to_string(&path) {
                Ok(content) => match serde_json::from_str(&content) {
                    Ok(config) => {
                        info!("Config loaded from {:?}", path);
                        return config;
                    }
                    Err(e) => {
                        error!("Failed to parse config: {}", e);
                    }
                },
                Err(e) => {
                    error!("Failed to read config file: {}", e);
                }
            }
        }

        // Create default config
        let config = Self::default();
        if let Err(e) = config.save() {
            error!("Failed to save default config: {}", e);
        }
        config
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::config_path();

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(self)?;
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
        let _ = fs::create_dir_all(parent);
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

#[cfg(not(target_os = "android"))]
pub fn init_android_paths(_app_handle: &tauri::AppHandle) {
    // 非 Android 平台无需初始化
}
