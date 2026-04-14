// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tracing::{info, error, warn};

use crate::config_service::ConfigService;
use crate::error::AppError;
use super::image_processor;
use super::providers;

const QUEUE_CAPACITY: usize = 32;
const AIEDIT_SUBDIR: &str = "AIEdit";

struct AiEditTask {
    file_path: PathBuf,
    result_tx: Option<oneshot::Sender<Result<PathBuf, AppError>>>,
}

pub struct AiEditService {
    sender: mpsc::Sender<AiEditTask>,
}

impl AiEditService {
    pub fn new(config_service: Arc<ConfigService>) -> Self {
        let (sender, receiver) = mpsc::channel::<AiEditTask>(QUEUE_CAPACITY);
        let config_service_clone = config_service.clone();

        tokio::spawn(async move {
            worker_loop(receiver, config_service_clone).await;
        });

        Self { sender }
    }

    /// Auto-trigger: non-blocking enqueue.
    pub async fn on_file_uploaded(&self, file_path: PathBuf) {
        if let Err(e) = self.sender.try_send(AiEditTask {
            file_path,
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
                result_tx: Some(tx),
            })
            .await
            .map_err(|_| AppError::AiEditError("AI edit service shut down".to_string()))?;

        rx.await
            .map_err(|_| AppError::AiEditError("AI edit worker dropped the task".to_string()))?
    }
}

async fn worker_loop(mut receiver: mpsc::Receiver<AiEditTask>, config_service: Arc<ConfigService>) {
    info!("AI edit worker started");

    while let Some(task) = receiver.recv().await {
        let result = process_task(&task.file_path, &config_service).await;

        match result {
            Ok(ref output_path) => {
                info!(input = %task.file_path.display(), output = %output_path.display(), "AI edit completed");
            }
            Err(ref e) => {
                error!(input = %task.file_path.display(), error = %e, "AI edit failed");
            }
        }

        if let Some(tx) = task.result_tx {
            let _ = tx.send(result);
        }
    }

    info!("AI edit worker stopped");
}

async fn process_task(file_path: &Path, config_service: &ConfigService) -> Result<PathBuf, AppError> {
    let config = config_service.get()
        .map_err(|e| AppError::AiEditError(format!("Failed to read config: {}", e)))?;

    let ai_config = &config.ai_edit;

    if !ai_config.enabled {
        return Err(AppError::AiEditError("AI edit is disabled".to_string()));
    }

    if let super::config::ProviderConfig::SeedEdit(ref seed_config) = ai_config.provider {
        if seed_config.api_key.is_empty() {
            return Err(AppError::AiEditError("API Key is not configured".to_string()));
        }
    }

    let base64_image = image_processor::prepare_for_upload(file_path)
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

    let stem = file_path.file_stem()
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
    let now = std::time::SystemTime::now();
    let duration = now.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let secs = duration.as_secs();

    let days = secs / 86400;
    let time_secs = secs % 86400;
    let hours = time_secs / 3600;
    let minutes = (time_secs % 3600) / 60;
    let seconds = time_secs % 60;

    let (year, month, day) = days_to_ymd(days);

    format!("{:04}{:02}{:02}_{:02}{:02}{:02}", year, month, day, hours, minutes, seconds)
}

fn days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    let mut year = 1970u64;
    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }

    let leap = is_leap_year(year);
    let month_days = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

    let mut month = 1u64;
    for &md in &month_days {
        if days < md {
            break;
        }
        days -= md;
        month += 1;
    }

    (year, month, days + 1)
}

fn is_leap_year(year: u64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}
