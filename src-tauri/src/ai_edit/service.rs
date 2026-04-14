// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use chrono::Local;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tauri::{AppHandle, Manager};
use tracing::{info, warn, debug};

use crate::config_service::ConfigService;
use crate::error::AppError;
use crate::file_index::FileIndexService;
use super::image_processor;
use super::providers;

const QUEUE_CAPACITY: usize = 32;
const AIEDIT_SUBDIR: &str = "AIEdit";

struct AiEditTask {
    file_path: PathBuf,
    is_auto_trigger: bool,
    result_tx: Option<oneshot::Sender<Result<PathBuf, AppError>>>,
}

pub struct AiEditService {
    sender: mpsc::Sender<AiEditTask>,
}

impl AiEditService {
    pub fn new(app_handle: AppHandle, config_service: Arc<ConfigService>) -> Self {
        let (sender, receiver) = mpsc::channel::<AiEditTask>(QUEUE_CAPACITY);
        let config_service_clone = config_service.clone();

        tokio::spawn(async move {
            worker_loop(receiver, app_handle, config_service_clone).await;
        });

        Self { sender }
    }

    /// Auto-trigger: non-blocking enqueue.
    pub async fn on_file_uploaded(&self, file_path: PathBuf) {
        if let Err(e) = self.sender.try_send(AiEditTask {
            file_path,
            is_auto_trigger: true,
            result_tx: None,
        }) {
            warn!("AI edit queue full, dropping task: {}", e);
        }
    }

    /// Manual trigger: enqueue and wait for result.
    pub async fn edit_single(&self, file_path: PathBuf) -> Result<PathBuf, AppError> {
        let (tx, rx) = oneshot::channel();

        self.sender
            .send(AiEditTask {
                file_path,
                is_auto_trigger: false,
                result_tx: Some(tx),
            })
            .await
            .map_err(|_| AppError::AiEditError("AI edit service shut down".to_string()))?;

        rx.await
            .map_err(|_| AppError::AiEditError("AI edit worker dropped the task".to_string()))?
    }
}

async fn worker_loop(mut receiver: mpsc::Receiver<AiEditTask>, app_handle: AppHandle, config_service: Arc<ConfigService>) {
    info!("AI edit worker started");

    while let Some(task) = receiver.recv().await {
        let result = process_task(&task, &config_service).await;

        match result {
            Ok(ref output_path) => {
                info!(input = %task.file_path.display(), output = %output_path.display(), "AI edit completed");

                // Index the new file so it appears in gallery
                if let Some(file_index) = app_handle.try_state::<Arc<FileIndexService>>() {
                    if let Err(e) = file_index.add_file(output_path.clone()).await {
                        warn!(path = %output_path.display(), error = %e, "Failed to index AI-edited file");
                    }
                }
            }
            Err(ref e) => {
                if task.result_tx.is_some() {
                    warn!(input = %task.file_path.display(), error = %e, "AI edit failed");
                }
            }
        }

        if let Some(tx) = task.result_tx {
            let _ = tx.send(result);
        }
    }

    info!("AI edit worker stopped");
}

async fn process_task(task: &AiEditTask, config_service: &ConfigService) -> Result<PathBuf, AppError> {
    let config = config_service.get()
        .map_err(|e| AppError::AiEditError(format!("Failed to read config: {}", e)))?;

    let ai_config = &config.ai_edit;

    if !ai_config.enabled {
        return Err(AppError::AiEditError("AI edit is disabled".to_string()));
    }

    // Auto-triggered tasks require auto_edit to be enabled
    if task.is_auto_trigger && !ai_config.auto_edit {
        debug!(file = %task.file_path.display(), "Auto-edit disabled, skipping");
        return Err(AppError::AiEditError("Auto-edit is disabled".to_string()));
    }

    let super::config::ProviderConfig::SeedEdit(ref seed_config) = ai_config.provider;
    if seed_config.api_key.is_empty() {
        return Err(AppError::AiEditError("API Key is not configured".to_string()));
    }

    let base64_image = image_processor::prepare_for_upload(&task.file_path)
        .map_err(|e| AppError::AiEditError(format!("Image preprocessing failed: {}", e)))?;

    let provider = providers::create_provider(&ai_config.provider)?;
    let prompt = if ai_config.prompt.is_empty() {
        "提升画质，使照片更清晰"
    } else {
        &ai_config.prompt
    };
    let image_bytes = provider.edit_image(&base64_image, prompt).await?;

    let output_dir = config.save_path.join(AIEDIT_SUBDIR);
    tokio::fs::create_dir_all(&output_dir).await
        .map_err(|e| AppError::AiEditError(format!("Failed to create AIEdit directory: {}", e)))?;

    let stem = task.file_path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("image");
    let datetime = chrono_now_string();
    let output_filename = format!("{}_AIEdit_{}.jpg", stem, datetime);
    let output_path = output_dir.join(&output_filename);

    tokio::fs::write(&output_path, &image_bytes).await
        .map_err(|e| AppError::AiEditError(format!("Failed to write edited image: {}", e)))?;

    Ok(output_path)
}

fn chrono_now_string() -> String {
    Local::now().format("%Y%m%d_%H%M%S").to_string()
}
