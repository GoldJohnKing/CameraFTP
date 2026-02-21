use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tracing::{error, info, warn};
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub save_path_uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub save_path_raw: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub save_path_display: Option<String>,
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
            save_path_uri: None,
            save_path_raw: None,
            save_path_display: None,
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

    // Android 11+ 对外部存储目录(/sdcard/Android/data/)访问受限
    // 使用应用内部存储目录 /data/data/<package>/files/ 确保始终可访问
    let internal_dir = app_handle
        .path()
        .data_dir()
        .unwrap_or_else(|_| PathBuf::from("/data/data/com.gjk.cameraftpcompanion/files"));

    let default_save_path = internal_dir.join("ftp_uploads");
    let config_path = internal_dir.join("config.json");

    // 确保目录存在
    let _ = fs::create_dir_all(&default_save_path);
    let _ = fs::create_dir_all(&internal_dir);

    // 尝试加载现有配置
    let mut config = AppConfig::load();

    // Android 10+ (API 29+) 引入 Scoped Storage，应用不能直接写入公共目录
    // 默认使用应用私有目录（不需要特殊权限）
    // 如果用户想要使用公共目录（如 DCIM），需要在设置中手动选择并开启权限
    if config.save_path.to_string_lossy().is_empty() {
        config.save_path = default_save_path.clone();
        let _ = config.save();
    } else {
        // 验证现有路径是否可写
        let test_file = config.save_path.join(".write_test");
        match fs::File::create(&test_file) {
            Ok(_) => {
                let _ = fs::remove_file(&test_file);
            }
            Err(_) => {
                // 路径不可写，回退到应用私有目录
                warn!("Save path is not writable, falling back to app private directory");
                config.save_path = default_save_path.clone();
                let _ = config.save();
            }
        }
    }

    let final_save_path = if config.save_path.to_string_lossy().is_empty() {
        default_save_path
    } else {
        config.save_path.clone()
    };

    set_android_paths(final_save_path.clone(), config_path.clone());
    info!(
        "Android paths initialized: save={:?}, config={:?}",
        final_save_path, config_path
    );
}

#[cfg(not(target_os = "android"))]
pub fn init_android_paths(_app_handle: &tauri::AppHandle) {
    // 非 Android 平台无需初始化
}
