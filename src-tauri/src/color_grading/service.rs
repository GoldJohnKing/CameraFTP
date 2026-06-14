// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tauri::{AppHandle, Emitter};

use crate::config::AutoColorGradingConfig;
use crate::config_service::ConfigService;
use crate::error::AppError;
use crate::image_utils;
use crate::utils::batch_state::BatchState;
use super::progress::ColorGradingEvent;
use super::presets::find_preset;

static GLOBAL_CG_SERVICE: OnceLock<Arc<ColorGradingService>> = OnceLock::new();

struct ColorGradingTask {
    input_path: PathBuf,
    lut_id: String,
    metering_mode: String,
    ev_offset: f32,
}

/// Holds the channel sender and task handle for a lazily-spawned worker.
/// The worker spawns on first task and stays alive for the app's lifetime —
/// blocking on `recv().await` costs ~0 CPU/battery, so idle destruction would
/// only waste the cached state (and risk TLS re-handshake overhead on respawn).
struct ColorGradingWorkerHandle {
    sender: mpsc::Sender<ColorGradingTask>,
    _join: tauri::async_runtime::JoinHandle<()>,
}

pub struct ColorGradingService {
    config_service: Arc<ConfigService>,
    app_handle: AppHandle,
    worker: tokio::sync::Mutex<Option<ColorGradingWorkerHandle>>,
    queue_depth: Arc<AtomicU32>,
    cancel_token: Arc<std::sync::Mutex<CancellationToken>>,
}

impl ColorGradingService {
    pub fn set_global(self: &Arc<Self>) {
        let _ = GLOBAL_CG_SERVICE.set(Arc::clone(self));
    }

    pub fn get_global() -> &'static Arc<Self> {
        GLOBAL_CG_SERVICE.get().expect("ColorGradingService global not initialized")
    }

    pub fn new(app_handle: AppHandle, config_service: Arc<ConfigService>) -> Self {
        Self {
            config_service,
            app_handle,
            worker: tokio::sync::Mutex::new(None),
            queue_depth: Arc::new(AtomicU32::new(0)),
            cancel_token: Arc::new(std::sync::Mutex::new(CancellationToken::new())),
        }
    }

    /// Lazily spawn the worker on first use, or respawn after the worker exits
    /// (panic or shutdown — detected via `sender.is_closed()`). Workers do not
    /// have an idle-timeout; they run for the app's lifetime once spawned.
    /// Returns a cloned sender for the caller to enqueue tasks.
    async fn ensure_worker(&self) -> mpsc::Sender<ColorGradingTask> {
        let mut guard = self.worker.lock().await;
        let needs_spawn = match guard.as_ref() {
            None => true,
            Some(h) => h.sender.is_closed(),
        };
        if needs_spawn {
            let (sender, receiver) = mpsc::channel::<ColorGradingTask>(16);
            let app_handle_clone = self.app_handle.clone();
            let queue_depth_clone = Arc::clone(&self.queue_depth);
            let cancel_token_clone = Arc::clone(&self.cancel_token);
            let join = tauri::async_runtime::spawn(async move {
                worker_loop(receiver, app_handle_clone, queue_depth_clone, cancel_token_clone).await;
            });
            *guard = Some(ColorGradingWorkerHandle {
                sender: sender.clone(),
                _join: join,
            });
            sender
        } else {
            guard.as_ref().unwrap().sender.clone()
        }
    }

    pub async fn enqueue(&self, file_paths: Vec<PathBuf>, lut_id: String, metering_mode: String, ev_offset: f32) -> Result<(), AppError> {
        let preset = find_preset(&lut_id)
            .ok_or_else(|| AppError::ColorGradingError(format!("Unknown LUT preset: {}", lut_id)))?;

        let sender = self.ensure_worker().await;
        let total = file_paths.len() as u32;
        if sender.is_closed() {
            return Err(AppError::ColorGradingError("Color grading queue is closed".to_string()));
        }
        self.queue_depth.fetch_add(total, Ordering::Relaxed);

        let mut sent = 0u32;
        for path in file_paths {
            match sender.send(ColorGradingTask {
                input_path: path,
                lut_id: preset.id.clone(),
                metering_mode: metering_mode.clone(),
                ev_offset,
            }).await {
                Ok(()) => sent += 1,
                Err(_) => {
                    self.queue_depth.fetch_sub(total - sent, Ordering::Relaxed);
                    return Err(AppError::ColorGradingError("Failed to enqueue task".to_string()));
                }
            }
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

        if !should_auto_color_grade(auto_cg, &file_path) {
            return;
        }

        let cg = auto_cg.unwrap();
        if let Err(e) = self.enqueue(
            vec![file_path.clone()],
            cg.preset_id.clone(),
            cg.metering_mode.clone(),
            cg.ev_offset,
        ).await {
            tracing::warn!("Auto color grading enqueue failed for {}: {}", file_path.display(), e);
        }
    }
}

/// Pure predicate: should auto color grading trigger for this file + config?
pub(crate) fn should_auto_color_grade(
    config: Option<&AutoColorGradingConfig>,
    file_path: &std::path::Path,
) -> bool {
    let _cg = match config {
        Some(cg) if cg.enabled && !cg.preset_id.is_empty() => cg,
        _ => return false,
    };
    image_utils::is_raw_file(file_path)
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
            biased;

            _ = cancel_token.cancelled() => {
                drain_pending_tasks(&mut receiver, &queue_depth);
                if state.processed_count() > 0 {
                    emit_done(&mut state, &app_handle, true);
                }
                continue;
            }
            t = receiver.recv() => match t {
                Some(t) => t,
                None => {
                    drain_pending_tasks(&mut receiver, &queue_depth);
                    if state.processed_count() > 0 {
                        emit_done(&mut state, &app_handle, true);
                    }
                    break;
                }
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

    let output_path = super::output::color_grading_output_path(&task.input_path, &preset.id)?;
    let result_path = output_path.to_string_lossy().into_owned();

    let lut_data = super::lut_data::get_lut_data(&preset.id)?;
    let lib = super::ffi::RawAlchemyLib::get()?;

    let lensfun_path = super::resources::get_resources()
        .ok()
        .map(|r| r.lensfun_db_dir.to_string_lossy().into_owned());

    let input_path = task.input_path.clone();
    let log_space = preset.log_space.clone();
    let metering_mode = task.metering_mode.clone();
    let ev_offset = task.ev_offset;

    tokio::task::spawn_blocking(move || {
        lib.process_file_with_lut(
            &input_path,
            &output_path,
            Some(&log_space),
            &lut_data,
            lensfun_path.as_deref(),
            ev_offset,
            &metering_mode,
        )
    }).await.map_err(|e| AppError::ColorGradingError(format!("Blocking task failed: {}", e)))??;

    Ok(result_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn enabled_cg() -> AutoColorGradingConfig {
        AutoColorGradingConfig { enabled: true, ..Default::default() }
    }

    #[test]
    fn should_auto_color_grade_enabled_raw_file() {
        assert!(should_auto_color_grade(Some(&enabled_cg()), Path::new("photo.nef")));
        assert!(should_auto_color_grade(Some(&enabled_cg()), Path::new("photo.CR3")));
    }

    #[test]
    fn should_auto_color_grade_disabled_even_for_raw() {
        let disabled = AutoColorGradingConfig { enabled: false, ..Default::default() };
        assert!(!should_auto_color_grade(Some(&disabled), Path::new("photo.nef")));
    }

    #[test]
    fn should_auto_color_grade_non_raw_even_if_enabled() {
        assert!(!should_auto_color_grade(Some(&enabled_cg()), Path::new("photo.jpg")));
        assert!(!should_auto_color_grade(Some(&enabled_cg()), Path::new("photo.mp4")));
    }

    #[test]
    fn should_auto_color_grade_requires_nonempty_preset() {
        let empty_preset = AutoColorGradingConfig { enabled: true, preset_id: String::new(), ..Default::default() };
        assert!(!should_auto_color_grade(Some(&empty_preset), Path::new("photo.nef")));
    }

    #[test]
    fn should_auto_color_grade_returns_false_when_no_config() {
        assert!(!should_auto_color_grade(None, Path::new("photo.nef")));
    }
}

