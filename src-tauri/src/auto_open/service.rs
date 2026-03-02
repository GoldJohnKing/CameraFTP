use std::path::PathBuf;
#[cfg(target_os = "windows")]
use std::sync::Arc;
#[cfg(target_os = "windows")]
use tokio::sync::Mutex;
#[cfg(target_os = "windows")]
use tauri::{Emitter, Manager};
use tauri::AppHandle;
#[cfg(target_os = "windows")]
use tracing::error;

#[cfg(target_os = "windows")]
use crate::config::{AppConfig, ImageOpenMethod};
use crate::config::PreviewWindowConfig;
use crate::error::AppError;

/// Macro to wrap errors with context message
macro_rules! wrap_err {
    ($result:expr, $msg:expr) => {
        $result.map_err(|e| AppError::Other(format!("{}: {}", $msg, e)))?
    };
}

pub struct AutoOpenService {
    #[allow(dead_code)]
    app_handle: AppHandle,
    #[cfg(target_os = "windows")]
    config: Arc<Mutex<PreviewWindowConfig>>,
}

impl AutoOpenService {
    pub fn new(app_handle: AppHandle) -> Self {
        #[cfg(target_os = "windows")]
        {
            let config = AppConfig::load().preview_config;
            Self {
                app_handle,
                config: Arc::new(Mutex::new(config)),
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            Self { app_handle }
        }
    }

    /// 处理文件上传事件
    pub async fn on_file_uploaded(&self, _file_path: PathBuf) -> Result<(), AppError> {
        #[cfg(target_os = "windows")]
        {
            let config = self.config.lock().await.clone();
            
            if !config.enabled {
                return Ok(());
            }

            match &config.method {
                ImageOpenMethod::BuiltInPreview => {
                    // 创建或更新预览窗口
                    self.open_or_update_preview_window(&_file_path, config.auto_bring_to_front).await?;
                }
                ImageOpenMethod::SystemDefault => {
                    crate::auto_open::windows::open_with_default(&_file_path)?;
                }
                ImageOpenMethod::WindowsPhotos => {
                    crate::auto_open::windows::open_with_photos(&_file_path)?;
                }
                ImageOpenMethod::Custom => {
                    if let Some(program_path) = &config.custom_path {
                        crate::auto_open::windows::open_with_program(&_file_path, program_path)?;
                    }
                }
            }
        }
        
        // Android 上暂时不支持自动打开
        Ok(())
    }

    /// 根据配置打开图片（用于手动触发）
    pub async fn open_image(&self, _file_path: &PathBuf) -> Result<(), AppError> {
        #[cfg(target_os = "windows")]
        {
            let config = self.config.lock().await.clone();
            
            match &config.method {
                ImageOpenMethod::BuiltInPreview => {
                    self.open_or_update_preview_window(_file_path, true).await?;
                }
                ImageOpenMethod::SystemDefault => {
                    crate::auto_open::windows::open_with_default(_file_path)?;
                }
                ImageOpenMethod::WindowsPhotos => {
                    crate::auto_open::windows::open_with_photos(_file_path)?;
                }
                ImageOpenMethod::Custom => {
                    if let Some(program_path) = &config.custom_path {
                        crate::auto_open::windows::open_with_program(_file_path, program_path)?;
                    }
                }
            }
        }
        
        Ok(())
    }

    /// 创建或更新预览窗口（仅 Windows）
    #[cfg(target_os = "windows")]
    async fn open_or_update_preview_window(
        &self, 
        file_path: &PathBuf, 
        bring_to_front: bool
    ) -> Result<(), AppError> {
        let event = PreviewEvent {
            file_path: file_path.to_string_lossy().to_string(),
            bring_to_front,
        };

        // 检查预览窗口是否已存在
        if let Some(window) = self.app_handle.get_webview_window("preview") {
            // 窗口已存在，发送事件更新图片
            wrap_err!(
                window.emit::<serde_json::Value>("preview-image", serde_json::to_value(&event).unwrap()),
                "Failed to emit preview event"
            );
            
            // 如果需要置顶
            if bring_to_front {
                wrap_err!(window.set_focus(), "Failed to focus window");
                wrap_err!(window.set_always_on_top(true), "Failed to set always on top");
                // 短暂置顶后恢复
                let window_clone = window.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                    let _ = window_clone.set_always_on_top(false);
                });
            }
        } else {
            // 创建新窗口
            let window = wrap_err!(
                tauri::WebviewWindowBuilder::new(
                    &self.app_handle,
                    "preview",
                    tauri::WebviewUrl::App("/preview".into())
                )
                .title("图片预览")
                .inner_size(1024.0, 768.0)
                .center()
                .resizable(true)
                .visible(true)
                .build(),
                "Failed to create preview window"
            );
            
            // 延迟发送事件，确保窗口已加载
            let event_clone = event.clone();
            let window_clone = window.clone();
            tokio::spawn(async move {
                tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
                let _ = window_clone.emit("preview-image", event_clone);
            });

            // 如果需要置顶
            if bring_to_front {
                wrap_err!(window.set_focus(), "Failed to focus window");
                wrap_err!(window.set_always_on_top(true), "Failed to set always on top");
                let window_clone = window.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                    let _ = window_clone.set_always_on_top(false);
                });
            }
        }

        Ok(())
    }

    /// 更新配置（仅 Windows）
    #[cfg(target_os = "windows")]
    pub async fn update_config(&self, new_config: PreviewWindowConfig) {
        let mut config = self.config.lock().await;
        *config = new_config.clone();
        
        // 持久化到配置文件
        let mut app_config = AppConfig::load();
        app_config.preview_config = new_config.clone();
        if let Err(e) = app_config.save() {
            error!("Failed to save preview config: {}", e);
        }
        
        // 广播配置变化事件给所有窗口
        let event = ConfigChangedEvent {
            config: new_config,
        };
        if let Err(e) = self.app_handle.emit("preview-config-changed", event) {
            error!("Failed to emit config changed event: {}", e);
        }
    }

    /// 更新配置（Android 空实现）
    #[cfg(not(target_os = "windows"))]
    pub async fn update_config(&self, _new_config: PreviewWindowConfig) {
        // Android 上暂时不支持
    }

    /// 获取当前配置（仅 Windows）
    #[cfg(target_os = "windows")]
    pub async fn get_config(&self) -> PreviewWindowConfig {
        self.config.lock().await.clone()
    }

    /// 获取当前配置（Android 返回默认）
    #[cfg(not(target_os = "windows"))]
    pub async fn get_config(&self) -> PreviewWindowConfig {
        PreviewWindowConfig::default()
    }
}

#[cfg(target_os = "windows")]
#[derive(Clone, serde::Serialize)]
pub struct PreviewEvent {
    pub file_path: String,
    pub bring_to_front: bool,
}

#[cfg(target_os = "windows")]
#[derive(Clone, serde::Serialize)]
pub struct ConfigChangedEvent {
    pub config: PreviewWindowConfig,
}