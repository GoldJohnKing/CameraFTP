// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tauri::{AppHandle, Emitter};

use crate::config_service::ConfigService;
use crate::error::AppError;
use super::progress::LutFilterProgressEvent;
use super::presets::find_preset;

struct LutFilterTask {
    input_path: PathBuf,
    lut_id: String,
}

pub struct LutFilterService {
    config_service: Arc<ConfigService>,
    app_handle: AppHandle,
    sender: mpsc::Sender<LutFilterTask>,
    queue_depth: Arc<AtomicU32>,
    cancel_token: Arc<tokio::sync::Mutex<CancellationToken>>,
}

impl LutFilterService {
    pub fn new(app_handle: AppHandle, config_service: Arc<ConfigService>) -> Self {
        let (sender, receiver) = mpsc::channel::<LutFilterTask>(16);
        let queue_depth = Arc::new(AtomicU32::new(0));
        let cancel_token = Arc::new(tokio::sync::Mutex::new(CancellationToken::new()));

        let app_handle_clone = app_handle.clone();
        let queue_depth_clone = queue_depth.clone();
        let cancel_token_clone = cancel_token.clone();

        tauri::async_runtime::spawn(async move {
            worker_loop(receiver, app_handle_clone, queue_depth_clone, cancel_token_clone).await;
        });

        Self { config_service, app_handle, sender, queue_depth, cancel_token }
    }

    pub async fn enqueue(&self, file_paths: Vec<PathBuf>, lut_id: String) -> Result<(), AppError> {
        let preset = find_preset(&lut_id)
            .ok_or_else(|| AppError::LutFilterError(format!("Unknown LUT preset: {}", lut_id)))?;

        for path in file_paths {
            let depth = self.queue_depth.fetch_add(1, Ordering::Relaxed);
            let _ = self.app_handle.emit("lut-filter-progress",
                &LutFilterProgressEvent::Progress {
                    current: depth + 1,
                    total: depth + 1,
                    file_name: path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
                    failed_count: 0,
                });

            self.sender.send(LutFilterTask {
                input_path: path,
                lut_id: preset.id.clone(),
            }).await.map_err(|e| AppError::LutFilterError(format!("Queue send failed: {}", e)))?;
        }
        Ok(())
    }

    pub fn cancel(&self) {
        let token = self.cancel_token.clone();
        tauri::async_runtime::spawn(async move {
            let guard = token.lock().await;
            guard.cancel();
            drop(guard);
            let mut guard = token.lock().await;
            *guard = CancellationToken::new();
        });
    }

    /// Auto-trigger: check config + RAW extension, then enqueue.
    pub async fn on_file_uploaded(&self, file_path: PathBuf) {
        let config = self.config_service.get().ok();
        let should_enqueue = config.as_ref()
            .and_then(|c| c.auto_lut.as_ref())
            .map(|lut| lut.enabled && !lut.preset_lut_id.is_empty())
            .unwrap_or(false);

        if !should_enqueue || !is_raw_file_path(&file_path) {
            return;
        }

        let lut_id = config
            .and_then(|c| c.auto_lut.map(|lut| lut.preset_lut_id))
            .unwrap_or_default();

        if let Err(e) = self.enqueue(vec![file_path.clone()], lut_id).await {
            tracing::warn!("Auto LUT filter enqueue failed for {}: {}", file_path.display(), e);
        }
    }
}

async fn worker_loop(
    mut receiver: mpsc::Receiver<LutFilterTask>,
    app_handle: AppHandle,
    queue_depth: Arc<AtomicU32>,
    cancel_token: Arc<tokio::sync::Mutex<CancellationToken>>,
) {
    let mut completed_count: u32 = 0;
    let mut failed_count: u32 = 0;
    let mut failed_files: Vec<String> = Vec::new();
    let mut output_files: Vec<String> = Vec::new();

    while let Some(task) = receiver.recv().await {
        // Check cancellation
        {
            let token = cancel_token.lock().await;
            if token.is_cancelled() {
                let _ = app_handle.emit("lut-filter-progress", &LutFilterProgressEvent::Done {
                    total: completed_count + failed_count,
                    failed_count,
                    failed_files: failed_files.clone(),
                    output_files: output_files.clone(),
                    cancelled: true,
                });
                while receiver.try_recv().is_ok() {}
                completed_count = 0;
                failed_count = 0;
                failed_files.clear();
                output_files.clear();
                continue;
            }
        }

        let total = completed_count + failed_count + queue_depth.load(Ordering::Relaxed) as u32;
        let current = completed_count + failed_count + 1;
        let file_name = task.input_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        let _ = app_handle.emit("lut-filter-progress", &LutFilterProgressEvent::Progress {
            current,
            total,
            file_name: file_name.clone(),
            failed_count,
        });

        match process_single_file(&task).await {
            Ok(output_path) => {
                completed_count += 1;
                let _ = app_handle.emit("lut-filter-progress", &LutFilterProgressEvent::Completed {
                    current,
                    total,
                    file_name: file_name.clone(),
                    failed_count,
                    output_path: output_path.clone(),
                });
                output_files.push(output_path);
            }
            Err(e) => {
                failed_count += 1;
                tracing::error!("LUT filter failed for {}: {}", file_name, e);
                let _ = app_handle.emit("lut-filter-progress", &LutFilterProgressEvent::Failed {
                    current,
                    total,
                    file_name: file_name.clone(),
                    error: e.to_string(),
                    failed_count,
                });
                failed_files.push(file_name);
            }
        }

        queue_depth.fetch_sub(1, Ordering::Relaxed);

        if queue_depth.load(Ordering::Relaxed) == 0 {
            let _ = app_handle.emit("lut-filter-progress", &LutFilterProgressEvent::Done {
                total: completed_count + failed_count,
                failed_count,
                failed_files: failed_files.clone(),
                output_files: output_files.clone(),
                cancelled: false,
            });
            completed_count = 0;
            failed_count = 0;
            failed_files.clear();
            output_files.clear();
        }
    }
}

async fn process_single_file(task: &LutFilterTask) -> Result<String, AppError> {
    let preset = find_preset(&task.lut_id)
        .ok_or_else(|| AppError::LutFilterError(format!("Unknown LUT: {}", task.lut_id)))?;

    let parent = task.input_path.parent()
        .ok_or_else(|| AppError::LutFilterError("No parent directory".into()))?;
    let stem = task.input_path.file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "output".into());
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let output_dir = parent.join("LutFilter");
    tokio::fs::create_dir_all(&output_dir).await
        .map_err(|e| AppError::LutFilterError(format!("Failed to create output dir: {}", e)))?;
    let output_name = format!("{}_{}_{}.jpg", stem, preset.id, timestamp);
    let output_path = output_dir.join(output_name);

    let lut_data = super::lut_data::get_lut_data(&preset.id)?;
    let lib = super::ffi::RawAlchemyLib::get()?;

    let lensfun_path = super::resources::get_resources()
        .ok()
        .map(|r| r.lensfun_db_dir.to_string_lossy().into_owned());

    lib.process_file_with_lut(
        &task.input_path,
        &output_path,
        Some(&preset.log_space),
        lut_data,
        lensfun_path.as_deref(),
    )?;

    Ok(output_path.to_string_lossy().into_owned())
}

fn is_raw_file_path(path: &std::path::Path) -> bool {
    let ext = path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    matches!(ext.as_str(),
        "nef" | "nrw" | "cr2" | "cr3" | "arw" | "sr2" | "raf" |
        "orf" | "rw2" | "pef" | "dng" | "x3f" | "raw" | "srw"
    )
}
