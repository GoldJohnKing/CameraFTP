use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::{AppHandle, Emitter};
use tracing::error;

use crate::config::{AppConfig, ImageOpenMethod, PreviewWindowConfig};
use crate::error::AppError;

pub struct AutoOpenService {
    app_handle: AppHandle,
    config: Arc<Mutex<PreviewWindowConfig>>,
}

impl AutoOpenService {
    pub fn new(app_handle: AppHandle) -> Self {
        let config = AppConfig::load().preview_config;
        Self {
            app_handle,
            config: Arc::new(Mutex::new(config)),
        }
    }

    /// 处理文件上传事件
    pub async fn on_file_uploaded(&self, file_path: PathBuf) -> Result<(), AppError> {
        let config = self.config.lock().await;
        
        if !config.enabled {
            return Ok(());
        }

        match config.method {
            ImageOpenMethod::BuiltInPreview => {
                // 发送事件到前端，打开/更新预览窗口
                self.emit_preview_event(file_path, config.auto_bring_to_front).await?;
            }
            ImageOpenMethod::SystemDefault => {
                self.open_with_system_default(&file_path).await?;
            }
            ImageOpenMethod::WindowsPhotos => {
                self.open_with_windows_photos(&file_path).await?;
            }
            ImageOpenMethod::Custom(ref program_path) => {
                self.open_with_custom_program(&file_path, program_path).await?;
            }
        }

        Ok(())
    }

    /// 更新配置
    pub async fn update_config(&self, new_config: PreviewWindowConfig) {
        let mut config = self.config.lock().await;
        *config = new_config.clone();
        
        // 持久化到配置文件
        let mut app_config = AppConfig::load();
        app_config.preview_config = new_config;
        if let Err(e) = app_config.save() {
            error!("Failed to save preview config: {}", e);
        }
    }

    /// 获取当前配置
    pub async fn get_config(&self) -> PreviewWindowConfig {
        self.config.lock().await.clone()
    }

    async fn emit_preview_event(&self, file_path: PathBuf, bring_to_front: bool) -> Result<(), AppError> {
        let event = PreviewEvent {
            file_path: file_path.to_string_lossy().to_string(),
            bring_to_front,
        };
        
        self.app_handle.emit("preview-image", event)
            .map_err(|e| AppError::Other(format!("Failed to emit preview event: {}", e)))?;
        
        Ok(())
    }

    #[cfg(target_os = "windows")]
    async fn open_with_system_default(&self, file_path: &PathBuf) -> Result<(), AppError> {
        crate::auto_open::windows::open_with_default(file_path)
    }

    #[cfg(target_os = "windows")]
    async fn open_with_windows_photos(&self, file_path: &PathBuf) -> Result<(), AppError> {
        crate::auto_open::windows::open_with_photos(file_path)
    }

    #[cfg(target_os = "windows")]
    async fn open_with_custom_program(&self, file_path: &PathBuf, program: &str) -> Result<(), AppError> {
        crate::auto_open::windows::open_with_program(file_path, program)
    }
    
    #[cfg(not(target_os = "windows"))]
    async fn open_with_system_default(&self, _file_path: &PathBuf) -> Result<(), AppError> {
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    async fn open_with_windows_photos(&self, _file_path: &PathBuf) -> Result<(), AppError> {
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    async fn open_with_custom_program(&self, _file_path: &PathBuf, _program: &str) -> Result<(), AppError> {
        Ok(())
    }
}

#[derive(Clone, serde::Serialize)]
struct PreviewEvent {
    file_path: String,
    bring_to_front: bool,
}
