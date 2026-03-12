// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

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
#[cfg(target_os = "windows")]
use crate::constants::{
    PREVIEW_WINDOW_WIDTH, PREVIEW_WINDOW_HEIGHT, PREVIEW_EMIT_DELAY_MS,
    PREVIEW_ON_TOP_DURATION_SECS,
};

/// Macro to wrap errors with context message
#[cfg(target_os = "windows")]
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
            let config = AppConfig::load().preview_config.unwrap_or_default();
            Self {
                app_handle,
                config: Arc::new(Mutex::new(config)),
            }
        }
        #[cfg(target_os = "android")]
        {
            Self { app_handle }
        }
    }

    /// 处理文件上传事件
    pub async fn on_file_uploaded(&self, _file_path: PathBuf) -> Result<(), AppError> {
        #[cfg(target_os = "windows")]
        {
            // 检查是否启用自动预览
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
            let event_json = serde_json::to_value(&event)
                .map_err(|e| AppError::Other(format!("Failed to serialize event: {}", e)))?;
            wrap_err!(
                window.emit::<serde_json::Value>("preview-image", event_json),
                "Failed to emit preview event"
            );
            
            // 如果需要置顶
            if bring_to_front {
                self.setup_window_on_top(&window).await?;
            }
        } else {
            // 创建新窗口
            let window = self.create_preview_window().await?;
            
            // 设置事件监听和超时
            self.setup_preview_event_handling(&window, event).await;
            
            // 如果需要置顶
            if bring_to_front {
                self.setup_window_on_top(&window).await?;
            }
        }

        Ok(())
    }

    /// 创建预览窗口
    #[cfg(target_os = "windows")]
    async fn create_preview_window(&self) -> Result<tauri::WebviewWindow, AppError> {
        let window = wrap_err!(
            tauri::WebviewWindowBuilder::new(
                &self.app_handle,
                "preview",
                tauri::WebviewUrl::App("/preview".into())
            )
            .title("图片预览")
            .inner_size(PREVIEW_WINDOW_WIDTH, PREVIEW_WINDOW_HEIGHT)
            .center()
            .resizable(true)
            .visible(true)
            .build(),
            "Failed to create preview window"
        );
        
        Ok(window)
    }

    /// 设置预览窗口事件处理
    /// 使用固定延迟确保窗口已加载完成
    #[cfg(target_os = "windows")]
    async fn setup_preview_event_handling(
        &self,
        window: &tauri::WebviewWindow,
        event: PreviewEvent
    ) {
        let event_clone = event.clone();
        let window_clone = window.clone();
        
        // 延迟发送事件，确保窗口已加载
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(PREVIEW_EMIT_DELAY_MS)).await;
            // 窗口可能已关闭，忽略发送错误
            let _ = window_clone.emit("preview-image", event_clone);
        });
    }

    /// 设置窗口置顶
    #[cfg(target_os = "windows")]
    async fn setup_window_on_top(&self, window: &tauri::WebviewWindow) -> Result<(), AppError> {
        wrap_err!(window.set_focus(), "Failed to focus window");
        wrap_err!(window.set_always_on_top(true), "Failed to set always on top");
        
        // 短暂置顶后恢复
        let window_clone = window.clone();
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_secs(PREVIEW_ON_TOP_DURATION_SECS)).await;
            let _ = window_clone.set_always_on_top(false);
        });
        
        Ok(())
    }

    /// 更新配置（仅 Windows）
    #[cfg(target_os = "windows")]
    pub async fn update_config(&self, new_config: PreviewWindowConfig) {
        let mut config = self.config.lock().await;
        *config = new_config.clone();
        
        // 持久化到配置文件
        let mut app_config = AppConfig::load();
        app_config.preview_config = Some(new_config.clone());
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
    #[cfg(target_os = "android")]
    pub async fn update_config(&self, _new_config: PreviewWindowConfig) {
        // Android 上暂时不支持
    }

    /// 获取当前配置（仅 Windows）
    #[cfg(target_os = "windows")]
    pub async fn get_config(&self) -> PreviewWindowConfig {
        self.config.lock().await.clone()
    }

    /// 获取当前配置（Android 返回默认）
    #[cfg(target_os = "android")]
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
