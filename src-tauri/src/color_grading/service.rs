// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
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
    /// NN demosaic gate. Defaults to true on all platforms — NN is always
    /// attempted, with classical demosaic as the fallback on decode failure.
    /// Retained as a runtime knob for future per-device gating/telemetry.
    nn_enabled: Arc<AtomicBool>,
}

/// Platform default for the NN demosaic gate. NN is always attempted; a
/// decode failure falls back to classical demosaic in the C++ decodeRaw layer.
fn nn_enabled_default() -> bool {
    true
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
            nn_enabled: Arc::new(AtomicBool::new(nn_enabled_default())),
        }
    }

    /// Whether NN demosaic is currently enabled. Defaults to true on all
    /// platforms; may be flipped at runtime via `set_nn_enabled` for future
    /// per-device gating/telemetry.
    pub fn is_nn_enabled(&self) -> bool {
        self.nn_enabled.load(Ordering::Relaxed)
    }

    /// Update the NN demosaic gate at runtime (e.g. per-device gating/telemetry).
    /// The worker reads the current value on each file, so a flip takes effect
    /// for the next enqueued task without restarting the worker.
    pub fn set_nn_enabled(&self, enabled: bool) {
        self.nn_enabled.store(enabled, Ordering::Relaxed);
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
            self.queue_depth.store(0, Ordering::Relaxed);
            let (sender, receiver) = mpsc::channel::<ColorGradingTask>(16);
            let app_handle_clone = self.app_handle.clone();
            let queue_depth_clone = Arc::clone(&self.queue_depth);
            let cancel_token_clone = Arc::clone(&self.cancel_token);
            let nn_enabled_clone = Arc::clone(&self.nn_enabled);
            let join = tauri::async_runtime::spawn(async move {
                worker_loop(receiver, app_handle_clone, queue_depth_clone, cancel_token_clone, nn_enabled_clone).await;
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

    /// Cancel the current batch and arm a fresh token for future tasks.
    ///
    /// Aborts in-flight and queued work via the active token, then replaces it
    /// with a new uncancelled token. Safe to call repeatedly — redundant calls
    /// are silently absorbed because the worker only reacts to the first active
    /// cancellation. Tasks enqueued after this call use the fresh token.
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
    nn_enabled: Arc<AtomicBool>,
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
            r = process_single_file(&task, &nn_enabled) => Some(r),
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

/// Result of an NN-path attempt that errored, telling the router how to fall back.
enum FallbackDecision {
    /// NN session was ready but this file failed (likely transient/file-specific):
    /// retry this file via classical, but keep NN enabled for the next file.
    UseClassicalNoLatch,
    /// NN session is not ready (structural NPU unavailability): retry this file
    /// via classical AND disable NN for the rest of the session.
    UseClassicalAndLatch,
}

/// Classify an NN-path failure by whether the NN session is ready.
/// NN unavailability is stable for the process, so we latch it; per-file
/// transient errors don't latch.
fn classify_nn_failure(nn_ready: bool) -> FallbackDecision {
    if nn_ready {
        FallbackDecision::UseClassicalNoLatch
    } else {
        FallbackDecision::UseClassicalAndLatch
    }
}

async fn process_single_file(
    task: &ColorGradingTask,
    nn_enabled: &Arc<AtomicBool>,
) -> Result<String, AppError> {
    let preset = find_preset(&task.lut_id)
        .ok_or_else(|| AppError::ColorGradingError(format!("Unknown LUT: {}", task.lut_id)))?;

    let output_path = super::output::color_grading_output_path(&task.input_path, &preset.id)?;
    let result_path = output_path.to_string_lossy().into_owned();

    let lut_data = super::lut_data::get_lut_data(&preset.id)?;
    let lib = super::ffi::RawAlchemyLib::get()?;

    let lensfun_path = super::resources::get_resources()
        .ok()
        .map(|r| r.lensfun_db_dir.to_string_lossy().into_owned());

    // First attempt: NN if enabled, else skip straight to classical.
    if nn_enabled.load(Ordering::Relaxed) {
        match decode_once(lib, task, &output_path, &preset, &lut_data, lensfun_path.as_deref(), true).await {
            Ok(()) => return Ok(result_path),
            Err(nn_err) => {
                match classify_nn_failure(super::ffi::is_nn_ready()) {
                    FallbackDecision::UseClassicalAndLatch => {
                        tracing::warn!(
                            "NN unavailable (NPU not engaged); latching classical demosaic for this session"
                        );
                        nn_enabled.store(false, Ordering::Relaxed);
                    }
                    FallbackDecision::UseClassicalNoLatch => {
                        tracing::warn!(
                            "NN decode failed on ready session; retrying this file via classical: {}",
                            nn_err
                        );
                    }
                }
                // fall through to classical attempt below
            }
        }
    }

    // Classical attempt (always last resort). An error here is a real failure.
    decode_once(lib, task, &output_path, &preset, &lut_data, lensfun_path.as_deref(), false)
        .await
        .map(|_| result_path)
}

/// One decode+grade attempt with a fixed NN flag. Extracted so the fallback
/// router can call it twice (NN then classical) without duplicating the
/// argument plumbing.
async fn decode_once(
    lib: &'static Arc<super::ffi::RawAlchemyLib>,
    task: &ColorGradingTask,
    output_path: &std::path::Path,
    preset: &super::presets::ColorGradingPreset,
    lut_data: &Arc<super::lut_data::LutData>,
    lensfun_path: Option<&str>,
    enable_nn: bool,
) -> Result<(), AppError> {
    let input_path = task.input_path.clone();
    let log_space = preset.log_space.clone();
    let metering_mode = task.metering_mode.clone();
    let ev_offset = task.ev_offset;
    let output_path = output_path.to_path_buf();
    let lensfun_path = lensfun_path.map(|s| s.to_owned());
    let lut_data = Arc::clone(lut_data);
    tokio::task::spawn_blocking(move || {
        lib.process_file_with_lut(
            &input_path,
            &output_path,
            Some(&log_space),
            &lut_data,
            lensfun_path.as_deref(),
            ev_offset,
            &metering_mode,
            enable_nn,
        )
    })
    .await
    .map_err(|e| AppError::ColorGradingError(format!("Blocking task failed: {}", e)))??;
    Ok(())
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

    #[test]
    fn fallback_latches_only_when_nn_structurally_unavailable() {
        // NN was up (ready) but this file errored → fall back, do NOT latch.
        let d = classify_nn_failure(true);
        assert!(matches!(d, FallbackDecision::UseClassicalNoLatch));
        // NN structurally unavailable (not ready) → fall back AND latch.
        let d = classify_nn_failure(false);
        assert!(matches!(d, FallbackDecision::UseClassicalAndLatch));
    }
}

