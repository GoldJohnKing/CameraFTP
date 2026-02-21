use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tracing::{error, info};
use ts_rs::TS;

/// Android 存储路径（在应用初始化时设置）
#[cfg(target_os = "android")]
static ANDROID_SAVE_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);

#[cfg(target_os = "android")]
static ANDROID_CONFIG_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);

/// 设置 Android 存储路径（在应用初始化时调用）
#[cfg(target_os = "android")]
pub fn set_android_paths(save_path: PathBuf, config_path: PathBuf) {
    let mut save_guard = ANDROID_SAVE_PATH.lock().unwrap();
    let mut config_guard = ANDROID_CONFIG_PATH.lock().unwrap();
    *save_guard = Some(save_path);
    *config_guard = Some(config_path);
    info!(
        "Android paths set: save={:?}, config={:?}",
        save_guard, config_guard
    );
}

/// 获取 Android 保存路径
#[cfg(target_os = "android")]
fn get_android_save_path() -> PathBuf {
    ANDROID_SAVE_PATH
        .lock()
        .unwrap()
        .clone()
        .unwrap_or_else(|| {
            PathBuf::from("/sdcard/Android/data/com.gjk.cameraftpcompanion/files/ftp_uploads")
        })
}

/// 获取 Android 配置路径
#[cfg(target_os = "android")]
fn get_android_config_path() -> PathBuf {
    ANDROID_CONFIG_PATH
        .lock()
        .unwrap()
        .clone()
        .unwrap_or_else(|| {
            PathBuf::from("/sdcard/Android/data/com.gjk.cameraftpcompanion/files/config.json")
        })
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AppConfig {
    pub save_path: PathBuf,
    pub auto_open: bool,
    pub auto_open_program: Option<String>,
    pub port: u16,
    pub auto_select_port: bool,
    pub file_extensions: Vec<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            save_path: Self::default_pictures_dir(),
            auto_open: true,
            auto_open_program: None,
            port: 2121,
            auto_select_port: true,
            file_extensions: vec![
                "jpg".to_string(),
                "jpeg".to_string(),
                "raw".to_string(),
                "png".to_string(),
                "arw".to_string(),
                "cr2".to_string(),
                "nef".to_string(),
                "orf".to_string(),
                "rw2".to_string(),
            ],
        }
    }
}

impl AppConfig {
    fn default_pictures_dir() -> PathBuf {
        #[cfg(target_os = "android")]
        {
            get_android_save_path()
        }
        #[cfg(not(target_os = "android"))]
        {
            dirs::picture_dir().unwrap_or_else(|| PathBuf::from("./pictures"))
        }
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

    // 获取应用外部存储目录: /sdcard/Android/data/com.gjk.cameraftpcompanion/files/
    let external_dir = app_handle
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| PathBuf::from("/sdcard/Android/data/com.gjk.cameraftpcompanion/files"));

    let save_path = external_dir.join("ftp_uploads");
    let config_path = external_dir.join("config.json");

    // 确保目录存在
    let _ = fs::create_dir_all(&save_path);
    let _ = fs::create_dir_all(external_dir);

    set_android_paths(save_path, config_path);
    info!("Android paths initialized");
}

#[cfg(not(target_os = "android"))]
pub fn init_android_paths(_app_handle: &tauri::AppHandle) {
    // 非 Android 平台无需初始化
}
