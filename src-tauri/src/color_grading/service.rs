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
use crate::image_utils;
use crate::utils::batch_state::BatchState;
use super::progress::ColorGradingEvent;
use super::presets::find_preset;

struct ColorGradingTask {
    input_path: PathBuf,
    lut_id: String,
    use_auto_exposure: bool,
    metering_mode: String,
    manual_ev: f32,
}

pub struct ColorGradingService {
    config_service: Arc<ConfigService>,
    app_handle: AppHandle,
    sender: mpsc::Sender<ColorGradingTask>,
    queue_depth: Arc<AtomicU32>,
    cancel_token: Arc<std::sync::Mutex<CancellationToken>>,
}

impl ColorGradingService {
    pub fn new(app_handle: AppHandle, config_service: Arc<ConfigService>) -> Self {
        let (sender, receiver) = mpsc::channel::<ColorGradingTask>(16);
        let queue_depth = Arc::new(AtomicU32::new(0));
        let cancel_token = Arc::new(std::sync::Mutex::new(CancellationToken::new()));

        let app_handle_clone = app_handle.clone();
        let queue_depth_clone = queue_depth.clone();
        let cancel_token_clone = cancel_token.clone();

        tauri::async_runtime::spawn(async move {
            worker_loop(receiver, app_handle_clone, queue_depth_clone, cancel_token_clone).await;
        });

        Self { config_service, app_handle, sender, queue_depth, cancel_token }
    }

    pub async fn enqueue(&self, file_paths: Vec<PathBuf>, lut_id: String, use_auto_exposure: bool, metering_mode: String, manual_ev: f32) -> Result<(), AppError> {
        let preset = find_preset(&lut_id)
            .ok_or_else(|| AppError::ColorGradingError(format!("Unknown LUT preset: {}", lut_id)))?;

        for path in file_paths {
            self.queue_depth.fetch_add(1, Ordering::Relaxed);

            self.sender.send(ColorGradingTask {
                input_path: path,
                lut_id: preset.id.clone(),
                use_auto_exposure,
                metering_mode: metering_mode.clone(),
                manual_ev,
            }).await.map_err(|e| AppError::ColorGradingError(format!("Queue send failed: {}", e)))?;
        }

        let depth = self.queue_depth.load(Ordering::Relaxed);
        let _ = self.app_handle.emit("color-grading-progress", &ColorGradingEvent::Queued { queue_depth: depth });

        Ok(())
    }

    pub fn cancel(&self) {
        let mut guard = self.cancel_token.lock().unwrap_or_else(|e| e.into_inner());
        guard.cancel();
        *guard = CancellationToken::new();
    }

    /// Auto-trigger: check config + RAW extension, then enqueue.
    pub async fn on_file_uploaded(&self, file_path: PathBuf) {
        let config = self.config_service.get().ok();
        let auto_cg = config.as_ref()
            .and_then(|c| c.auto_color_grading.as_ref());

        let should_enqueue = auto_cg
            .map(|cg| cg.enabled && !cg.preset_id.is_empty())
            .unwrap_or(false);

        if !should_enqueue || !image_utils::is_raw_file(&file_path) {
            return;
        }

        let cg = auto_cg.unwrap();
        if let Err(e) = self.enqueue(
            vec![file_path.clone()],
            cg.preset_id.clone(),
            cg.use_auto_exposure,
            cg.metering_mode.clone(),
            cg.manual_ev,
        ).await {
            tracing::warn!("Auto color grading enqueue failed for {}: {}", file_path.display(), e);
        }
    }
}

async fn worker_loop(
    mut receiver: mpsc::Receiver<ColorGradingTask>,
    app_handle: AppHandle,
    queue_depth: Arc<AtomicU32>,
    cancel_token_arc: Arc<std::sync::Mutex<CancellationToken>>,
) {
    tracing::info!("Color grading worker started");

    let mut state = BatchState::default();

    fn emit_done(
        state: &mut BatchState,
        app_handle: &AppHandle,
        cancelled: bool,
    ) {
        let _ = app_handle.emit("color-grading-progress", &ColorGradingEvent::Done {
            total: state.processed_count(),
            failed_count: state.failed_count,
            failed_files: std::mem::take(&mut state.failed_files),
            output_files: std::mem::take(&mut state.output_files),
            cancelled,
        });
        state.reset();
    }

    fn drain_pending_tasks(
        receiver: &mut mpsc::Receiver<ColorGradingTask>,
        queue_depth: &AtomicU32,
    ) {
        while let Ok(_) = receiver.try_recv() {
            queue_depth.fetch_sub(1, Ordering::Relaxed);
        }
    }

    loop {
        let cancel_token = cancel_token_arc.lock().unwrap_or_else(|e| e.into_inner()).clone();

        if cancel_token.is_cancelled() {
            drain_pending_tasks(&mut receiver, &queue_depth);
            if state.processed_count() > 0 {
                emit_done(&mut state, &app_handle, true);
            }
            continue;
        }

        let task = tokio::select! {
            t = receiver.recv() => match t {
                Some(t) => t,
                None => {
                    drain_pending_tasks(&mut receiver, &queue_depth);
                    if state.processed_count() > 0 {
                        emit_done(&mut state, &app_handle, true);
                    }
                    break;
                }
            },
            _ = cancel_token.cancelled() => {
                drain_pending_tasks(&mut receiver, &queue_depth);
                if state.processed_count() > 0 {
                    emit_done(&mut state, &app_handle, true);
                }
                continue;
            }
        };

        queue_depth.fetch_sub(1, Ordering::Relaxed);

        let remaining = queue_depth.load(Ordering::Relaxed);
        let current = state.processed_count() + 1;
        let total = current + remaining;
        let file_name = task.input_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        let _ = app_handle.emit("color-grading-progress", &ColorGradingEvent::Progress {
            current,
            total,
            file_name: file_name.clone(),
            failed_count: state.failed_count,
        });

        let result = tokio::select! {
            r = process_single_file(&task) => Some(r),
            _ = cancel_token.cancelled() => {
                tracing::info!("Color grading cancelled before/during task processing");
                None
            }
        };

        match result {
            Some(Ok(output_path)) => {
                tracing::info!(input = %task.input_path.display(), output = %output_path, "Color grading completed");
                state.completed_count += 1;
                state.output_files.push(output_path.clone());

                let remaining = queue_depth.load(Ordering::Relaxed);
                let _ = app_handle.emit("color-grading-progress", &ColorGradingEvent::Completed {
                    current: state.processed_count(),
                    total: state.processed_count() + remaining,
                    file_name: file_name.clone(),
                    failed_count: state.failed_count,
                    output_path,
                });
            }
            Some(Err(ref e)) => {
                tracing::error!(input = %task.input_path.display(), error = %e, "Color grading failed");
                state.failed_count += 1;
                state.failed_files.push(file_name.clone());

                let remaining = queue_depth.load(Ordering::Relaxed);
                let _ = app_handle.emit("color-grading-progress", &ColorGradingEvent::Failed {
                    current: state.processed_count(),
                    total: state.processed_count() + remaining,
                    file_name: file_name.clone(),
                    error: e.to_string(),
                    failed_count: state.failed_count,
                });
            }
            None => {
                drain_pending_tasks(&mut receiver, &queue_depth);
                emit_done(&mut state, &app_handle, true);
                continue;
            }
        }

        if queue_depth.load(Ordering::Relaxed) == 0 && state.processed_count() > 0 {
            emit_done(&mut state, &app_handle, false);
        }
    }

    tracing::info!("Color grading worker stopped");
}

async fn process_single_file(task: &ColorGradingTask) -> Result<String, AppError> {
    let preset = find_preset(&task.lut_id)
        .ok_or_else(|| AppError::ColorGradingError(format!("Unknown LUT: {}", task.lut_id)))?;

    let parent = task.input_path.parent()
        .ok_or_else(|| AppError::ColorGradingError("No parent directory".into()))?;
    let stem = task.input_path.file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "output".into());
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let output_dir = parent.join("ColorGrading");
    tokio::fs::create_dir_all(&output_dir).await
        .map_err(|e| AppError::ColorGradingError(format!("Failed to create output dir: {}", e)))?;
    let output_name = format!("{}_{}_{}.jpg", stem, preset.id, timestamp);
    let output_path = output_dir.join(output_name);
    let result_path = output_path.to_string_lossy().into_owned();

    let lut_data = super::lut_data::get_lut_data(&preset.id)?;
    let lib = super::ffi::RawAlchemyLib::get()?;

    let lensfun_path = super::resources::get_resources()
        .ok()
        .map(|r| r.lensfun_db_dir.to_string_lossy().into_owned());

    let input_path = task.input_path.clone();
    let log_space = preset.log_space.clone();
    let metering_mode = task.metering_mode.clone();
    let use_auto_exposure = task.use_auto_exposure;
    let manual_ev = task.manual_ev;

    tokio::task::spawn_blocking(move || {
        lib.process_file_with_lut(
            &input_path,
            &output_path,
            Some(&log_space),
            &lut_data,
            lensfun_path.as_deref(),
            use_auto_exposure,
            &metering_mode,
            manual_ev,
        )
    }).await.map_err(|e| AppError::ColorGradingError(format!("Blocking task failed: {}", e)))??;

    Ok(result_path)
}

#[cfg(test)]
mod tests {
    use crate::config::AutoColorGradingConfig;

    #[test]
    fn auto_trigger_requires_enabled_and_preset() {
        let config = AutoColorGradingConfig::default();
        assert!(!config.enabled || !config.preset_id.is_empty());

        let enabled = AutoColorGradingConfig { enabled: true, ..Default::default() };
        assert!(enabled.enabled && !enabled.preset_id.is_empty());
    }

    #[test]
    fn auto_trigger_skips_non_raw() {
        let raw_path = std::path::PathBuf::from("/tmp/photo.nef");
        let jpg_path = std::path::PathBuf::from("/tmp/photo.jpg");
        assert!(crate::image_utils::is_raw_file(&raw_path));
        assert!(!crate::image_utils::is_raw_file(&jpg_path));
    }
}

