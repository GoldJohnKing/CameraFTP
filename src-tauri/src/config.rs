use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::fs;
use tracing::{error, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub save_path: PathBuf,
    pub auto_open: bool,
    pub auto_open_program: Option<String>,
    pub port: u16,
    pub file_extensions: Vec<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            save_path: Self::default_pictures_dir(),
            auto_open: true,
            auto_open_program: None,
            port: 21,
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
        dirs::picture_dir().unwrap_or_else(|| PathBuf::from("./pictures"))
    }
    
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .map(|d| d.join("camera-ftp-companion"))
            .unwrap_or_else(|| PathBuf::from("./config"))
            .join("config.json")
    }
    
    pub fn load() -> Self {
        let path = Self::config_path();
        
        if path.exists() {
            match fs::read_to_string(&path) {
                Ok(content) => {
                    match serde_json::from_str(&content) {
                        Ok(config) => {
                            info!("Config loaded from {:?}", path);
                            return config;
                        }
                        Err(e) => {
                            error!("Failed to parse config: {}", e);
                        }
                    }
                }
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