// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use chrono::Utc;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;
use tauri::{AppHandle, Emitter, Manager};
use tracing::{info, warn, debug};
use tokio_util::sync::CancellationToken;

use crate::config_service::ConfigService;
use crate::error::AppError;
use crate::file_index::FileIndexService;
use crate::utils::batch_state::BatchState;
use crate::utils::task_worker::{CancelGate, QueueDepth};
use super::image_processor;
use super::providers;

const MANUAL_QUEUE_CAPACITY: usize = 4;
const AUTO_QUEUE_CAPACITY: usize = 32;
const AIEDIT_SUBDIR: &str = "AIEdit";


struct AiEditTask {
    file_path: PathBuf,
    override_prompt: Option<String>,
    override_model: Option<String>,
}

/// Holds channel senders and task handle for a lazily-spawned worker.
/// The worker spawns on first task and stays alive for the app's lifetime —
/// blocking on `recv().await` costs ~0 CPU/battery, so idle destruction would
/// only waste the cached provider (HTTP client + TLS session).
struct AiEditWorkerHandle {
    manual_sender: mpsc::Sender<AiEditTask>,
    auto_sender: mpsc::Sender<AiEditTask>,
    _join: tauri::async_runtime::JoinHandle<()>,
}

pub struct AiEditService {
    config_service: Arc<ConfigService>,
    /// Used by `emit_queued()` and the `QueuedDropped` path in `on_file_uploaded()`.
    /// The worker_loop uses its own cloned AppHandle for all other event emissions.
    app_handle: AppHandle,
    worker: tokio::sync::Mutex<Option<AiEditWorkerHandle>>,
    queue_depth: QueueDepth,
    cancel_gate: CancelGate,
}

impl AiEditService {
    pub fn new(app_handle: AppHandle, config_service: Arc<ConfigService>) -> Self {
        Self {
            config_service,
            app_handle,
            worker: tokio::sync::Mutex::new(None),
            queue_depth: QueueDepth::new(),
            cancel_gate: CancelGate::new(),
        }
    }

    /// Lazily spawn the worker on first use, or respawn after the worker exits
    /// (panic or shutdown — detected via `sender.is_closed()`). Workers do not
    /// have an idle-timeout; they run for the app's lifetime once spawned.
    /// Returns cloned senders for the caller to enqueue tasks.
    async fn ensure_worker(&self) -> (mpsc::Sender<AiEditTask>, mpsc::Sender<AiEditTask>) {
        let mut guard = self.worker.lock().await;
        let needs_spawn = match guard.as_ref() {
            None => true,
            Some(h) => h.manual_sender.is_closed(),
        };
        if needs_spawn {
            self.queue_depth.reset();
            let (manual_sender, manual_receiver) = mpsc::channel::<AiEditTask>(MANUAL_QUEUE_CAPACITY);
            let (auto_sender, auto_receiver) = mpsc::channel::<AiEditTask>(AUTO_QUEUE_CAPACITY);
            let config_service_clone = Arc::clone(&self.config_service);
            let queue_depth_clone = self.queue_depth.clone();
            let cancel_gate_clone = self.cancel_gate.clone();
            let app_handle_clone = self.app_handle.clone();
            let join = tauri::async_runtime::spawn(async move {
                worker_loop(manual_receiver, auto_receiver, app_handle_clone, config_service_clone, queue_depth_clone, cancel_gate_clone).await;
            });
            *guard = Some(AiEditWorkerHandle {
                manual_sender: manual_sender.clone(),
                auto_sender: auto_sender.clone(),
                _join: join,
            });
            (manual_sender, auto_sender)
        } else {
            let h = guard.as_ref().unwrap();
            (h.manual_sender.clone(), h.auto_sender.clone())
        }
    }

    /// Auto-trigger: non-blocking enqueue.
    /// Checks `auto_edit` and non-empty prompt before enqueueing.
    pub async fn on_file_uploaded(&self, file_path: PathBuf) {
        let should_enqueue = self.config_service.get()
            .map(|c| c.ai_edit.auto_edit && !c.ai_edit.prompt.trim().is_empty())
            .unwrap_or(false);

        if !should_enqueue {
            return;
        }

        let (_, auto_sender) = self.ensure_worker().await;
        self.queue_depth.add(1);
        let task = AiEditTask {
            file_path,
            override_prompt: None,
            override_model: None,
        };
        if let Err(e) = auto_sender.try_send(task) {
            self.queue_depth.sub(1);
            let dropped_task = e.into_inner();
            warn!("AI edit queue full, dropping task: {}", dropped_task.file_path.display());

            let file_name = dropped_task.file_path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();
            let depth = self.queue_depth.get();
            if let Err(emit_err) = self.app_handle.emit("ai-edit-progress", &super::progress::AiEditProgressEvent::QueuedDropped {
                file_name,
                queue_depth: depth,
            }) {
                warn!(error = %emit_err, "Failed to emit ai-edit-progress QueuedDropped event");
            }
        } else {
            self.emit_queued();
        }
    }

    /// Manual batch enqueue (non-blocking, no result callback).
    pub async fn enqueue_manual(&self, file_path: PathBuf, override_prompt: Option<String>, override_model: Option<String>) -> Result<(), AppError> {
        let (manual_sender, _) = self.ensure_worker().await;
        self.queue_depth.add(1);
        if let Err(e) = manual_sender
            .send(AiEditTask {
                file_path,
                override_prompt,
                override_model,
            })
            .await
        {
            self.queue_depth.sub(1);
            return Err(AppError::AiEditError(format!("AI edit service shut down: {}", e)));
        } else {
            self.emit_queued();
        }
        Ok(())
    }

    /// Cancel the current batch and arm a fresh token for future tasks.
    /// Full contract lives on [`CancelGate::cancel_and_rearm`].
    pub fn cancel(&self) {
        self.cancel_gate.cancel_and_rearm();
    }

    fn emit_queued(&self) {
        let depth = self.queue_depth.get();
        if let Err(e) = self.app_handle.emit("ai-edit-progress", &super::progress::AiEditProgressEvent::Queued {
            queue_depth: depth,
        }) {
            warn!(error = %e, "Failed to emit ai-edit-progress Queued event");
        }
    }
}

enum SelectOutcome {
    Task(AiEditTask),
    Cancelled,
    AutoChannelClosed(AiEditTask),
    ShutDown,
}

async fn select_next_task(
    manual_rx: &mut mpsc::Receiver<AiEditTask>,
    auto_rx: &mut mpsc::Receiver<AiEditTask>,
    cancel_token: &CancellationToken,
) -> SelectOutcome {
    tokio::select! {
        biased;

        _ = cancel_token.cancelled() => SelectOutcome::Cancelled,

        task = manual_rx.recv() => match task {
            Some(t) => SelectOutcome::Task(t),
            None => SelectOutcome::ShutDown,
        },

        task = auto_rx.recv() => match task {
            Some(t) => SelectOutcome::Task(t),
            None => {
                // Auto channel closed — fall back to manual-only select
                tokio::select! {
                    _ = cancel_token.cancelled() => SelectOutcome::Cancelled,
                    task = manual_rx.recv() => match task {
                        Some(t) => SelectOutcome::AutoChannelClosed(t),
                        None => SelectOutcome::ShutDown,
                    },
                }
            }
        }
    }
}

/// Generic over the runtime so tests can drive the loop with a mock
/// `AppHandle<MockRuntime>` (production always passes `AppHandle<Wry>`).
async fn worker_loop<R: tauri::Runtime>(
    mut manual_rx: mpsc::Receiver<AiEditTask>,
    mut auto_rx: mpsc::Receiver<AiEditTask>,
    app_handle: AppHandle<R>,
    config_service: Arc<ConfigService>,
    queue_depth: QueueDepth,
    cancel_gate: CancelGate,
) {
    info!("AI edit worker started");

    let mut state = BatchState::default();

    let mut cached_provider: Option<Box<dyn providers::AiEditProvider>> = None;
    let mut cached_api_key: Option<String> = None;
    let mut cached_model: Option<String> = None;

    fn emit_batch_done<R: tauri::Runtime>(
        state: &mut BatchState,
        app_handle: &AppHandle<R>,
        cancelled: bool,
    ) {
        if let Err(e) = app_handle.emit("ai-edit-progress", &super::progress::AiEditProgressEvent::Done {
            total: state.processed_count(),
            failed_count: state.failed_count,
            failed_files: std::mem::take(&mut state.failed_files),
            output_files: std::mem::take(&mut state.output_files),
            cancelled,
        }) {
            warn!(error = %e, "Failed to emit ai-edit-progress Done event");
        }

        state.reset();
    }

    fn drain_pending_tasks(
        manual_rx: &mut mpsc::Receiver<AiEditTask>,
        auto_rx: &mut mpsc::Receiver<AiEditTask>,
        queue_depth: &QueueDepth,
    ) {
        while manual_rx.try_recv().is_ok() {
            queue_depth.sub(1);
        }
        while auto_rx.try_recv().is_ok() {
            queue_depth.sub(1);
        }
    }

    loop {
        let cancel_token = cancel_gate.current();

        // Fast path: drain pending manual tasks first (high priority)
        let task = if let Ok(task) = manual_rx.try_recv() {
            task
        } else {
            match select_next_task(&mut manual_rx, &mut auto_rx, &cancel_token).await {
                SelectOutcome::Task(task) => task,
                SelectOutcome::AutoChannelClosed(task) => {
                    // Auto channel closed mid-batch — emit Done if batch has pending results
                    if queue_depth.get() == 0 && state.processed_count() > 0 {
                        emit_batch_done(&mut state, &app_handle, false);
                    }
                    task
                }
                SelectOutcome::Cancelled => {
                    info!("AI edit worker cancelled while waiting");
                    drain_pending_tasks(&mut manual_rx, &mut auto_rx, &queue_depth);
                    emit_batch_done(&mut state, &app_handle, true);
                    continue;
                }
                SelectOutcome::ShutDown => {
                    drain_pending_tasks(&mut manual_rx, &mut auto_rx, &queue_depth);
                    emit_batch_done(&mut state, &app_handle, true);
                    break;
                }
            }
        };

        // Decrement queue depth BEFORE calculating progress (fixes off-by-one)
        queue_depth.sub(1);

        let remaining = queue_depth.get();
        let current = state.processed_count() + 1;
        let total = current + remaining;
        let file_name = task.file_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        if let Err(e) = app_handle.emit("ai-edit-progress", &super::progress::AiEditProgressEvent::Progress {
            current,
            total,
            file_name: file_name.clone(),
            failed_count: state.failed_count,
        }) {
            warn!(error = %e, "Failed to emit ai-edit-progress Progress event");
        }

        // Process task with cancel awareness: abort current task on cancel
        let result = tokio::select! {
            r = process_task(&task, &config_service, &mut cached_provider, &mut cached_api_key, &mut cached_model) => Some(r),
            _ = cancel_token.cancelled() => {
                info!("AI edit cancelled during task processing");
                None
            }
        };

        match result {
            Some(Ok(ref output_path)) => {
                info!(input = %task.file_path.display(), output = %output_path.display(), "AI edit completed");

                if let Some(file_index) = app_handle.try_state::<Arc<FileIndexService>>() {
                    if let Err(e) = file_index.add_file(output_path.clone()).await {
                        debug!(path = %output_path.display(), error = %e, "Failed to index AI-edited file");
                    }
                }

                state.completed_count += 1;
                let output_str = output_path.to_string_lossy().to_string();
                state.output_files.push(output_str.clone());

                let remaining = queue_depth.get();
                if let Err(e) = app_handle.emit("ai-edit-progress", &super::progress::AiEditProgressEvent::Completed {
                    current: state.processed_count(),
                    total: state.processed_count() + remaining,
                    file_name: file_name.clone(),
                    failed_count: state.failed_count,
                    output_path: Some(output_str),
                }) {
                    warn!(error = %e, "Failed to emit ai-edit-progress Completed event");
                }
            }
            Some(Err(ref e)) => {
                debug!(input = %task.file_path.display(), error = %e, "AI edit failed");

                state.failed_count += 1;
                state.failed_files.push(file_name.clone());

                let remaining = queue_depth.get();
                if let Err(e) = app_handle.emit("ai-edit-progress", &super::progress::AiEditProgressEvent::Failed {
                    current: state.processed_count(),
                    total: state.processed_count() + remaining,
                    file_name: file_name.clone(),
                    error: e.to_string(),
                    failed_count: state.failed_count,
                }) {
                    warn!(error = %e, "Failed to emit ai-edit-progress Failed event");
                }
            }
            None => {
                // Task was cancelled during processing
                drain_pending_tasks(&mut manual_rx, &mut auto_rx, &queue_depth);
                emit_batch_done(&mut state, &app_handle, true);
                continue;
            }
        }

        // Emit Done when queue is empty and batch is complete
        if queue_depth.get() == 0 && state.processed_count() > 0 {
            emit_batch_done(&mut state, &app_handle, false);
        }
    }

    info!("AI edit worker stopped");
}

async fn process_task(
    task: &AiEditTask,
    config_service: &ConfigService,
    cached_provider: &mut Option<Box<dyn providers::AiEditProvider>>,
    cached_api_key: &mut Option<String>,
    cached_model: &mut Option<String>,
) -> Result<PathBuf, AppError> {
    let config = config_service.get()
        .map_err(|e| AppError::AiEditError(format!("Failed to read config: {}", e)))?;

    let ai_config = &config.ai_edit;
    let super::config::ProviderConfig::SeedEdit(ref seed_config) = ai_config.provider;
    if seed_config.api_key.is_empty() {
        return Err(AppError::AiEditError("API Key is not configured".to_string()));
    }

    let file_path_clone = task.file_path.clone();
    let prepared = tokio::task::spawn_blocking(move || {
        let preprocessor = image_processor::create_preprocessor();
        preprocessor.prepare(&file_path_clone)
    }).await
        .map_err(|e| AppError::AiEditError(format!("Preprocessing task panicked: {}", e)))??;

    let current_api_key = seed_config.api_key.clone();
    let effective_model = task.override_model.as_deref()
        .unwrap_or(&seed_config.model)
        .to_string();

    if cached_api_key.as_ref() != Some(&current_api_key) || cached_model.as_ref() != Some(&effective_model) {
        let mut provider_config = ai_config.provider.clone();
        let super::config::ProviderConfig::SeedEdit(ref mut cfg) = provider_config;
        cfg.model = effective_model.clone();
        *cached_provider = Some(providers::create_provider(&provider_config)?);
        *cached_api_key = Some(current_api_key);
        *cached_model = Some(effective_model);
    }

    let provider = cached_provider.as_ref()
        .ok_or_else(|| AppError::AiEditError("No provider available".to_string()))?;

    let prompt = task.override_prompt.as_deref()
        .or_else(|| if ai_config.prompt.is_empty() { None } else { Some(&ai_config.prompt) })
        .ok_or_else(|| AppError::AiEditError("提示词不能为空，请先配置提示词".to_string()))?;
    let image_bytes = provider.edit_image(&prepared.base64_data, prepared.mime_type, prompt).await?;

    let output_dir = config.save_path.join(AIEDIT_SUBDIR);
    tokio::fs::create_dir_all(&output_dir).await
        .map_err(|e| AppError::AiEditError(format!("Failed to create AIEdit directory: {}", e)))?;

    let stem = task.file_path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("image");
    let datetime = chrono_now_string();

    let output_path = write_edited_image(&output_dir, stem, &datetime, &image_bytes).await?;

    Ok(output_path)
}

async fn write_edited_image(
    output_dir: &Path,
    stem: &str,
    datetime: &str,
    image_bytes: &[u8],
) -> Result<PathBuf, AppError> {
    let primary_name = format!("{}_AIEdit_{}.jpg", stem, datetime);
    let primary_path = output_dir.join(&primary_name);

    if try_write_exclusive(&primary_path, image_bytes).await.is_ok() {
        return Ok(primary_path);
    }

    for i in 1u32..=99 {
        let retry_name = format!("{}_AIEdit_{}_{}.jpg", stem, datetime, i);
        let retry_path = output_dir.join(&retry_name);
        if try_write_exclusive(&retry_path, image_bytes).await.is_ok() {
            return Ok(retry_path);
        }
    }

    Err(AppError::AiEditError(
        "Failed to write edited image: too many file name collisions".to_string(),
    ))
}

async fn try_write_exclusive(path: &Path, data: &[u8]) -> Result<(), std::io::Error> {
    let mut file = tokio::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .await?;
    file.write_all(data).await
}

/// Generates a timestamp string for output filenames.
/// Uses UTC to avoid chrono::Local panics on some Android devices where timezone data is unavailable.
fn chrono_now_string() -> String {
    Utc::now().format("%Y%m%d_%H%M%S%.3f").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chrono_now_string_includes_milliseconds() {
        let s = chrono_now_string();
        assert!(s.contains('.'), "Expected millisecond separator: got {}", s);
        let parts: Vec<&str> = s.split('.').collect();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[1].len(), 3, "Expected 3-digit milliseconds");
    }

    #[tokio::test]
    async fn write_edited_image_handles_collisions() {
        let dir = tempfile::TempDir::new().unwrap();
        let data = b"test-image-bytes";

        // Create a file that collides with the primary name
        let collision_path = dir.path().join("photo_AIEdit_20260101_120000.000.jpg");
        tokio::fs::write(&collision_path, b"existing").await.unwrap();

        let result = write_edited_image(dir.path(), "photo", "20260101_120000.000", data).await;
        assert!(result.is_ok(), "Should succeed with retry name");
        let output = result.unwrap();
        assert_ne!(output, collision_path, "Should use a different filename");
        assert!(output.file_name().unwrap().to_str().unwrap().contains("_1.jpg"));
    }

    #[test]
    fn prompt_selection_prefers_override_over_config() {
        // override_prompt > config.prompt (if non-empty) > None
        let override_prompt: Option<&str> = Some("override");
        let config_prompt = "config-prompt";

        let result = override_prompt
            .or_else(|| if config_prompt.is_empty() { None } else { Some(config_prompt) });
        assert_eq!(result, Some("override"));

        // Test fallback to config prompt
        let override_prompt: Option<&str> = None;
        let result = override_prompt
            .or_else(|| if config_prompt.is_empty() { None } else { Some(config_prompt) });
        assert_eq!(result, Some("config-prompt"));

        // Test error case: both empty
        let override_prompt: Option<&str> = None;
        let config_prompt = "";
        let result = override_prompt
            .or_else(|| if config_prompt.is_empty() { None } else { Some(config_prompt) });
        assert!(result.is_none());
    }

    // ---- Worker-loop tests (mock AppHandle via tauri::test) ----

    use crate::ai_edit::progress::AiEditProgressEvent;
    use std::sync::Mutex;
    use std::time::Duration;
    use tauri::Listener;

    fn make_task(name: &str) -> AiEditTask {
        AiEditTask {
            file_path: PathBuf::from(name),
            override_prompt: None,
            override_model: None,
        }
    }

    /// Collects deserialized `ai-edit-progress` events from the mock app.
    fn event_collector(handle: &tauri::AppHandle<tauri::test::MockRuntime>) -> Arc<Mutex<Vec<AiEditProgressEvent>>> {
        let events: Arc<Mutex<Vec<AiEditProgressEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&events);
        handle.listen("ai-edit-progress", move |e| {
            if let Ok(ev) = serde_json::from_str::<AiEditProgressEvent>(e.payload()) {
                sink.lock().unwrap().push(ev);
            }
        });
        events
    }

    /// Polls `probe` until it returns `Some` or the timeout elapses (panics).
    async fn wait_until<T>(timeout: Duration, mut probe: impl FnMut() -> Option<T>) -> T {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if let Some(value) = probe() {
                return value;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "timed out after {:?} waiting for condition",
                timeout
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    #[tokio::test]
    async fn select_next_task_prefers_manual_over_auto() {
        let (manual_tx, mut manual_rx) = mpsc::channel::<AiEditTask>(4);
        let (auto_tx, mut auto_rx) = mpsc::channel::<AiEditTask>(32);
        let token = CancellationToken::new();

        manual_tx.send(make_task("m.jpg")).await.unwrap();
        auto_tx.send(make_task("a.jpg")).await.unwrap();

        match select_next_task(&mut manual_rx, &mut auto_rx, &token).await {
            SelectOutcome::Task(t) => assert_eq!(t.file_path, PathBuf::from("m.jpg")),
            SelectOutcome::Cancelled => panic!("expected manual task, got Cancelled"),
            SelectOutcome::AutoChannelClosed(_) => panic!("expected manual task, got AutoChannelClosed"),
            SelectOutcome::ShutDown => panic!("expected manual task, got ShutDown"),
        }
    }

    #[tokio::test]
    async fn select_next_task_returns_auto_when_manual_idle() {
        let (_manual_tx, mut manual_rx) = mpsc::channel::<AiEditTask>(4);
        let (auto_tx, mut auto_rx) = mpsc::channel::<AiEditTask>(32);
        let token = CancellationToken::new();

        auto_tx.send(make_task("a.jpg")).await.unwrap();

        match select_next_task(&mut manual_rx, &mut auto_rx, &token).await {
            SelectOutcome::Task(t) => assert_eq!(t.file_path, PathBuf::from("a.jpg")),
            SelectOutcome::Cancelled => panic!("expected auto task, got Cancelled"),
            SelectOutcome::AutoChannelClosed(_) => panic!("expected auto task, got AutoChannelClosed"),
            SelectOutcome::ShutDown => panic!("expected auto task, got ShutDown"),
        }
    }

    #[tokio::test]
    async fn select_next_task_cancelled_wins_over_ready_channels() {
        let (manual_tx, mut manual_rx) = mpsc::channel::<AiEditTask>(4);
        let (auto_tx, mut auto_rx) = mpsc::channel::<AiEditTask>(32);
        let token = CancellationToken::new();

        manual_tx.send(make_task("m.jpg")).await.unwrap();
        auto_tx.send(make_task("a.jpg")).await.unwrap();
        token.cancel();

        // `biased` select must poll the cancel branch before ready receivers
        match select_next_task(&mut manual_rx, &mut auto_rx, &token).await {
            SelectOutcome::Cancelled => {}
            SelectOutcome::Task(_) => panic!("expected Cancelled, got Task"),
            SelectOutcome::AutoChannelClosed(_) => panic!("expected Cancelled, got AutoChannelClosed"),
            SelectOutcome::ShutDown => panic!("expected Cancelled, got ShutDown"),
        }
    }

    #[tokio::test]
    async fn select_next_task_returns_shutdown_when_both_channels_closed() {
        let (manual_tx, mut manual_rx) = mpsc::channel::<AiEditTask>(4);
        let (auto_tx, mut auto_rx) = mpsc::channel::<AiEditTask>(32);
        let token = CancellationToken::new();

        drop(manual_tx);
        drop(auto_tx);

        match select_next_task(&mut manual_rx, &mut auto_rx, &token).await {
            SelectOutcome::ShutDown => {}
            SelectOutcome::Task(_) => panic!("expected ShutDown, got Task"),
            SelectOutcome::Cancelled => panic!("expected ShutDown, got Cancelled"),
            SelectOutcome::AutoChannelClosed(_) => panic!("expected ShutDown, got AutoChannelClosed"),
        }
    }

    #[tokio::test]
    async fn select_next_task_falls_back_to_manual_when_auto_closed() {
        let (manual_tx, mut manual_rx) = mpsc::channel::<AiEditTask>(4);
        let (auto_tx, mut auto_rx) = mpsc::channel::<AiEditTask>(32);
        let token = CancellationToken::new();

        drop(auto_tx);

        let select = tokio::spawn(async move {
            select_next_task(&mut manual_rx, &mut auto_rx, &token).await
        });

        // Park the spawned select in the inner (manual-only) fallback select
        tokio::task::yield_now().await;
        manual_tx.send(make_task("m.jpg")).await.unwrap();

        match select.await.unwrap() {
            SelectOutcome::AutoChannelClosed(t) => assert_eq!(t.file_path, PathBuf::from("m.jpg")),
            SelectOutcome::Task(_) => panic!("expected AutoChannelClosed, got Task"),
            SelectOutcome::Cancelled => panic!("expected AutoChannelClosed, got Cancelled"),
            SelectOutcome::ShutDown => panic!("expected AutoChannelClosed, got ShutDown"),
        }
    }

    #[tokio::test]
    async fn worker_loop_processes_tasks_and_emits_done_when_queue_drains() {
        let app = tauri::test::mock_app();
        let handle = app.handle().clone();
        let events = event_collector(&handle);

        let (manual_tx, manual_rx) = mpsc::channel::<AiEditTask>(4);
        let (auto_tx, auto_rx) = mpsc::channel::<AiEditTask>(32);
        let queue_depth = QueueDepth::new();
        let cancel_gate = CancelGate::new();

        let temp = tempfile::tempdir().unwrap();
        // Default config: empty API key → process_task fails fast, no network
        let config_service = Arc::new(ConfigService::new_with_path(temp.path().join("config.json")));

        let worker = tokio::spawn(worker_loop(
            manual_rx,
            auto_rx,
            handle,
            config_service,
            queue_depth.clone(),
            cancel_gate,
        ));

        // Enqueue 3 tasks mirroring the service contract (depth++ before send)
        for name in ["a.jpg", "b.jpg", "c.jpg"] {
            queue_depth.add(1);
            auto_tx.send(make_task(name)).await.unwrap();
        }

        wait_until(Duration::from_secs(10), || {
            let ev = events.lock().unwrap();
            ev.iter()
                .rev()
                .find(|e| matches!(e, AiEditProgressEvent::Done { cancelled: false, .. }))
                .cloned()
        })
        .await;

        assert_eq!(queue_depth.get(), 0, "queue depth must drain to 0");

        let ev = events.lock().unwrap();
        let failed_files: Vec<&str> = ev
            .iter()
            .filter_map(|e| match e {
                AiEditProgressEvent::Failed { file_name, .. } => Some(file_name.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(
            failed_files,
            vec!["a.jpg", "b.jpg", "c.jpg"],
            "tasks must be processed in FIFO enqueue order"
        );
        assert!(
            ev.iter().any(|e| matches!(
                e,
                AiEditProgressEvent::Failed { error, .. } if error.contains("API Key")
            )),
            "empty API key config must fail tasks: {:?}",
            ev
        );
        let done_count = ev
            .iter()
            .filter(|e| matches!(e, AiEditProgressEvent::Done { cancelled: false, total: 3, failed_count: 3, .. }))
            .count();
        assert_eq!(done_count, 1, "exactly one non-cancelled Done(3 failed) expected: {:?}", ev);
        drop(ev);

        drop(manual_tx);
        drop(auto_tx);
        let _ = tokio::time::timeout(Duration::from_secs(5), worker).await;
    }

    #[tokio::test]
    async fn worker_loop_cancel_drains_pending_tasks_and_recovers_with_fresh_token() {
        let app = tauri::test::mock_app();
        let handle = app.handle().clone();
        let events = event_collector(&handle);

        let (manual_tx, manual_rx) = mpsc::channel::<AiEditTask>(4);
        let (auto_tx, auto_rx) = mpsc::channel::<AiEditTask>(32);
        let queue_depth = QueueDepth::new();
        let cancel_gate = CancelGate::new();

        let temp = tempfile::tempdir().unwrap();
        let config_service = Arc::new(ConfigService::new_with_path(temp.path().join("config.json")));

        let worker = tokio::spawn(worker_loop(
            manual_rx,
            auto_rx,
            handle,
            config_service,
            queue_depth.clone(),
            cancel_gate.clone(),
        ));

        // Park the idle worker inside select_next_task
        tokio::task::yield_now().await;

        // Mirror AiEditService::cancel(): cancel the active token, arm a fresh one
        cancel_gate.cancel_and_rearm();

        // Queue 3 tasks AFTER cancellation; the biased select must drain them,
        // never process them.
        for name in ["p1.jpg", "p2.jpg", "p3.jpg"] {
            queue_depth.add(1);
            auto_tx.try_send(make_task(name)).expect("channel has capacity");
        }

        wait_until(Duration::from_secs(10), || {
            let drained = queue_depth.get() == 0;
            let done_cancelled = events.lock().unwrap().iter().any(|e| matches!(
                e,
                AiEditProgressEvent::Done { cancelled: true, total: 0, .. }
            ));
            (drained && done_cancelled).then_some(())
        })
        .await;

        {
            let ev = events.lock().unwrap();
            assert!(
                !ev.iter().any(|e| matches!(e, AiEditProgressEvent::Failed { .. })),
                "drained tasks must not be processed: {:?}",
                ev
            );
            assert!(
                !ev.iter().any(|e| matches!(e, AiEditProgressEvent::Progress { file_name, .. } if file_name.starts_with("p"))),
                "drained tasks must not emit progress: {:?}",
                ev
            );
        }

        // The fresh token keeps the worker alive: a new task is processed normally
        queue_depth.add(1);
        auto_tx.send(make_task("d.jpg")).await.unwrap();

        wait_until(Duration::from_secs(10), || {
            let ev = events.lock().unwrap();
            ev.iter()
                .rev()
                .find(|e| matches!(e, AiEditProgressEvent::Done { cancelled: false, total: 1, failed_count: 1, .. }))
                .cloned()
        })
        .await;
        assert_eq!(queue_depth.get(), 0);

        drop(manual_tx);
        drop(auto_tx);
        let _ = tokio::time::timeout(Duration::from_secs(5), worker).await;
    }
}
