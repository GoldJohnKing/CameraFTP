use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};
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
        let config = self.config.lock().await.clone();
        
        if !config.enabled {
            return Ok(());
        }

        match &config.method {
            ImageOpenMethod::BuiltInPreview => {
                // 创建或更新预览窗口
                self.open_or_update_preview_window(&file_path, config.auto_bring_to_front).await?;
            }
            ImageOpenMethod::SystemDefault => {
                #[cfg(target_os = "windows")]
                {
                    crate::auto_open::windows::open_with_default(&file_path)?;
                }
            }
            ImageOpenMethod::WindowsPhotos => {
                #[cfg(target_os = "windows")]
                {
                    crate::auto_open::windows::open_with_photos(&file_path)?;
                }
            }
            ImageOpenMethod::Custom => {
                #[cfg(target_os = "windows")]
                {
                    if let Some(program_path) = &config.custom_path {
                        crate::auto_open::windows::open_with_program(&file_path, program_path)?;
                    }
                }
            }
        }

        Ok(())
    }

    /// 根据配置打开图片（用于手动触发）
    pub async fn open_image(&self, file_path: &PathBuf) -> Result<(), AppError> {
        let config = self.config.lock().await.clone();
        
        match &config.method {
            ImageOpenMethod::BuiltInPreview => {
                self.open_or_update_preview_window(file_path, true).await?;
            }
            ImageOpenMethod::SystemDefault => {
                #[cfg(target_os = "windows")]
                crate::auto_open::windows::open_with_default(file_path)?;
            }
            ImageOpenMethod::WindowsPhotos => {
                #[cfg(target_os = "windows")]
                crate::auto_open::windows::open_with_photos(file_path)?;
            }
            ImageOpenMethod::Custom => {
                #[cfg(target_os = "windows")]
                if let Some(program_path) = &config.custom_path {
                    crate::auto_open::windows::open_with_program(file_path, program_path)?;
                }
            }
        }

        Ok(())
    }

    /// 创建或更新预览窗口
    async fn open_or_update_preview_window(&self, file_path: &PathBuf, bring_to_front: bool) -> Result<(), AppError> {
        let event = PreviewEvent {
            file_path: file_path.to_string_lossy().to_string(),
            bring_to_front,
        };

        // 检查预览窗口是否已存在
        if let Some(window) = self.app_handle.get_webview_window("preview") {
            // 窗口已存在，发送事件更新图片
            window.emit("preview-image", event.clone())
                .map_err(|e| AppError::Other(format!("Failed to emit preview event: {}", e)))?;
            
            // 如果需要置顶
            if bring_to_front {
                window.set_focus()
                    .map_err(|e| AppError::Other(format!("Failed to focus window: {}", e)))?;
                window.set_always_on_top(true)
                    .map_err(|e| AppError::Other(format!("Failed to set always on top: {}", e)))?;
                // 短暂置顶后恢复
                let window_clone = window.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                    let _ = window_clone.set_always_on_top(false);
                });
            }
        } else {
            // 创建新窗口
            let window = tauri::WebviewWindowBuilder::new(
                &self.app_handle,
                "preview",
                tauri::WebviewUrl::App("/preview".into())
            )
            .title("图片预览")
            .inner_size(1024.0, 768.0)
            .center()
            .resizable(true)
            .visible(true)
            .build()
            .map_err(|e| AppError::Other(format!("Failed to create preview window: {}", e)))?;
            
            // 延迟发送事件，确保窗口已加载
            let event_clone = event.clone();
            let window_clone = window.clone();
            tokio::spawn(async move {
                tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
                let _ = window_clone.emit("preview-image", event_clone);
            });

            // 如果需要置顶
            if bring_to_front {
                window.set_focus()
                    .map_err(|e| AppError::Other(format!("Failed to focus window: {}", e)))?;
                window.set_always_on_top(true)
                    .map_err(|e| AppError::Other(format!("Failed to set always on top: {}", e)))?;
                let window_clone = window.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                    let _ = window_clone.set_always_on_top(false);
                });
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
}

#[derive(Clone, serde::Serialize)]
pub struct PreviewEvent {
    pub file_path: String,
    pub bring_to_front: bool,
}
