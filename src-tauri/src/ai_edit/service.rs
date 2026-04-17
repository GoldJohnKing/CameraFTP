// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use chrono::Utc;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::io::AsyncWriteExt;
use tokio::sync::{mpsc, oneshot};
use tauri::{AppHandle, Emitter, Manager};
use tracing::{info, warn, debug};
use tokio_util::sync::CancellationToken;

use crate::config_service::ConfigService;
use crate::error::AppError;
use crate::file_index::FileIndexService;
use super::image_processor;
use super::providers;

const MANUAL_QUEUE_CAPACITY: usize = 4;
const AUTO_QUEUE_CAPACITY: usize = 32;
const AIEDIT_SUBDIR: &str = "AIEdit";

/// Placeholder is no longer used — prompt must be explicitly configured
const _DEFAULT_EDIT_PROMPT: &str = "提升画质，使照片更清晰";

struct AiEditTask {
    file_path: PathBuf,
    override_prompt: Option<String>,
    override_model: Option<String>,
    result_tx: Option<oneshot::Sender<Result<PathBuf, AppError>>>,
}

pub struct AiEditService {
    config_service: Arc<ConfigService>,
    app_handle: AppHandle,
    manual_sender: mpsc::Sender<AiEditTask>,
    auto_sender: mpsc::Sender<AiEditTask>,
    queue_depth: Arc<AtomicU32>,
    cancel_token: Arc<Mutex<CancellationToken>>,
}

impl AiEditService {
    pub fn new(app_handle: AppHandle, config_service: Arc<ConfigService>) -> Self {
        let (manual_sender, manual_receiver) = mpsc::channel::<AiEditTask>(MANUAL_QUEUE_CAPACITY);
        let (auto_sender, auto_receiver) = mpsc::channel::<AiEditTask>(AUTO_QUEUE_CAPACITY);
        let config_service_clone = config_service.clone();
        let queue_depth = Arc::new(AtomicU32::new(0));
        let queue_depth_clone = queue_depth.clone();
        let cancel_token = Arc::new(Mutex::new(CancellationToken::new()));
        let cancel_token_clone = cancel_token.clone();
        let app_handle_clone = app_handle.clone();

        tauri::async_runtime::spawn(async move {
            worker_loop(manual_receiver, auto_receiver, app_handle_clone, config_service_clone, queue_depth_clone, cancel_token_clone).await;
        });

        Self {
            config_service,
            app_handle,
            manual_sender,
            auto_sender,
            queue_depth,
            cancel_token,
        }
    }

    /// Auto-trigger: non-blocking enqueue.
    /// Checks `enabled`, `auto_edit`, and non-empty prompt before enqueueing.
    pub async fn on_file_uploaded(&self, file_path: PathBuf) {
        let should_enqueue = self.config_service.get()
            .map(|c| c.ai_edit.enabled && c.ai_edit.auto_edit && !c.ai_edit.prompt.trim().is_empty())
            .unwrap_or(false);

        if !should_enqueue {
            return;
        }

        self.queue_depth.fetch_add(1, Ordering::Relaxed);
        if let Err(e) = self.auto_sender.try_send(AiEditTask {
            file_path,
            override_prompt: None,
            override_model: None,
            result_tx: None,
        }) {
            self.queue_depth.fetch_sub(1, Ordering::Relaxed);
            warn!("AI edit queue full, dropping task: {}", e);
        } else {
            self.emit_queued();
        }
    }

    /// Manual trigger: enqueue and wait for result.
    pub async fn edit_single(&self, file_path: PathBuf, override_prompt: Option<String>, override_model: Option<String>) -> Result<PathBuf, AppError> {
        self.queue_depth.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();

        if self.manual_sender
            .send(AiEditTask {
                file_path,
                override_prompt,
                override_model,
                result_tx: Some(tx),
            })
            .await
            .is_err()
        {
            self.queue_depth.fetch_sub(1, Ordering::Relaxed);
            return Err(AppError::AiEditError("AI edit service shut down".to_string()));
        } else {
            self.emit_queued();
        }

        rx.await
            .map_err(|_| AppError::AiEditError("AI edit worker dropped the task".to_string()))?
    }

    /// Manual batch enqueue (non-blocking, no result callback).
    pub async fn enqueue_manual(&self, file_path: PathBuf, override_prompt: Option<String>, override_model: Option<String>) -> Result<(), AppError> {
        self.queue_depth.fetch_add(1, Ordering::Relaxed);
        if let Err(e) = self.manual_sender
            .send(AiEditTask {
                file_path,
                override_prompt,
                override_model,
                result_tx: None,
            })
            .await
        {
            self.queue_depth.fetch_sub(1, Ordering::Relaxed);
            return Err(AppError::AiEditError(format!("AI edit service shut down: {}", e)));
        } else {
            self.emit_queued();
        }
        Ok(())
    }

    pub fn queue_len(&self) -> u32 {
        self.queue_depth.load(Ordering::Relaxed)
    }

    pub fn cancel(&self) {
        let mut guard = self.cancel_token.lock().unwrap();
        guard.cancel();
        *guard = CancellationToken::new();
    }

    fn emit_queued(&self) {
        let depth = self.queue_depth.load(Ordering::Relaxed);
        if let Err(e) = self.app_handle.emit("ai-edit-progress", &super::progress::AiEditProgressEvent::Queued {
            queue_depth: depth,
        }) {
            warn!(error = %e, "Failed to emit ai-edit-progress Queued event");
        }
    }
}

async fn worker_loop(
    mut manual_rx: mpsc::Receiver<AiEditTask>,
    mut auto_rx: mpsc::Receiver<AiEditTask>,
    app_handle: AppHandle,
    config_service: Arc<ConfigService>,
    queue_depth: Arc<AtomicU32>,
    cancel_token_arc: Arc<Mutex<CancellationToken>>,
) {
    info!("AI edit worker started");

    struct WorkerState {
        completed_count: u32,
        failed_count: u32,
        failed_files: Vec<String>,
        output_files: Vec<String>,
        batch_total: u32,
    }

    impl WorkerState {
        fn processed_count(&self) -> u32 {
            self.completed_count + self.failed_count
        }

        fn reset(&mut self) {
            self.completed_count = 0;
            self.failed_count = 0;
            self.failed_files.clear();
            self.output_files.clear();
            self.batch_total = 0;
        }
    }

    let mut state = WorkerState {
        completed_count: 0,
        failed_count: 0,
        failed_files: Vec::new(),
        output_files: Vec::new(),
        batch_total: 0,
    };

    let mut cached_provider: Option<Box<dyn providers::AiEditProvider>> = None;
    let mut cached_api_key: Option<String> = None;
    let mut cached_model: Option<String> = None;

    fn emit_batch_done(
        state: &mut WorkerState,
        app_handle: &AppHandle,
    ) {
        if let Err(e) = app_handle.emit("ai-edit-progress", &super::progress::AiEditProgressEvent::Done {
            total: state.processed_count(),
            failed_count: state.failed_count,
            failed_files: std::mem::take(&mut state.failed_files),
            output_files: std::mem::take(&mut state.output_files),
        }) {
            warn!(error = %e, "Failed to emit ai-edit-progress Done event");
        }

        state.reset();
    }

    fn drain_pending_tasks(
        manual_rx: &mut mpsc::Receiver<AiEditTask>,
        auto_rx: &mut mpsc::Receiver<AiEditTask>,
        queue_depth: &AtomicU32,
    ) {
        while let Ok(task) = manual_rx.try_recv() {
            queue_depth.fetch_sub(1, Ordering::Relaxed);
            if let Some(tx) = task.result_tx {
                let _ = tx.send(Err(AppError::AiEditError("AI edit cancelled".to_string())));
            }
        }
        while let Ok(task) = auto_rx.try_recv() {
            queue_depth.fetch_sub(1, Ordering::Relaxed);
            if let Some(tx) = task.result_tx {
                let _ = tx.send(Err(AppError::AiEditError("AI edit cancelled".to_string())));
            }
        }
    }

    loop {
        let cancel_token = cancel_token_arc.lock().unwrap().clone();

        if cancel_token.is_cancelled() {
            info!("AI edit worker cancelled");
            drain_pending_tasks(&mut manual_rx, &mut auto_rx, &queue_depth);
            emit_batch_done(&mut state, &app_handle);
            continue;
        }

        // Fast path: drain pending manual tasks first (high priority)
        let task = if let Ok(task) = manual_rx.try_recv() {
            task
        } else {
            // Slow path: wait on either channel, manual has priority via biased select
            tokio::select! {
                biased;

                _ = cancel_token.cancelled() => {
                    info!("AI edit worker cancelled while waiting");
                    drain_pending_tasks(&mut manual_rx, &mut auto_rx, &queue_depth);
                    emit_batch_done(&mut state, &app_handle);
                    continue;
                }

                task = manual_rx.recv() => {
                    match task {
                        Some(t) => t,
                        None => {
                            // Manual channel closed; drain pending then finish
                            drain_pending_tasks(&mut manual_rx, &mut auto_rx, &queue_depth);
                            emit_batch_done(&mut state, &app_handle);
                            break;
                        },
                    }
                }
                task = auto_rx.recv() => {
                    match task {
                        Some(t) => t,
                        None => {
                            // Auto channel closed; wait for manual tasks or shutdown
                            if queue_depth.load(Ordering::Relaxed) == 0 && state.processed_count() > 0 {
                                emit_batch_done(&mut state, &app_handle);
                            }
                            tokio::select! {
                                _ = cancel_token.cancelled() => {
                                    drain_pending_tasks(&mut manual_rx, &mut auto_rx, &queue_depth);
                                    emit_batch_done(&mut state, &app_handle);
                                    continue;
                                }
                                task = manual_rx.recv() => {
                                    match task {
                                        Some(t) => t,
                                        None => {
                                            emit_batch_done(&mut state, &app_handle);
                                            break;
                                        },
                                    }
                                }
                            }
                        }
                    }
                }
            }
        };

        // Decrement queue depth BEFORE calculating progress (fixes off-by-one)
        queue_depth.fetch_sub(1, Ordering::Relaxed);

        let remaining = queue_depth.load(Ordering::Relaxed);
        let current = state.processed_count() + 1;
        let total = current + remaining;
        state.batch_total = total;
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

                let remaining = queue_depth.load(Ordering::Relaxed);
                if let Err(e) = app_handle.emit("ai-edit-progress", &super::progress::AiEditProgressEvent::Completed {
                    current: state.processed_count(),
                    total: state.processed_count() + remaining,
                    file_name: file_name.clone(),
                    failed_count: state.failed_count,
                    output_path: Some(output_str),
                }) {
                    warn!(error = %e, "Failed to emit ai-edit-progress Completed event");
                }

                if let Some(tx) = task.result_tx {
                    let _ = tx.send(Ok(output_path.clone()));
                }
            }
            Some(Err(ref e)) => {
                if task.result_tx.is_some() {
                    warn!(input = %task.file_path.display(), error = %e, "AI edit failed");
                } else {
                    debug!(input = %task.file_path.display(), error = %e, "Auto AI edit failed");
                }

                state.failed_count += 1;
                state.failed_files.push(file_name.clone());

                let remaining = queue_depth.load(Ordering::Relaxed);
                if let Err(e) = app_handle.emit("ai-edit-progress", &super::progress::AiEditProgressEvent::Failed {
                    current: state.processed_count(),
                    total: state.processed_count() + remaining,
                    file_name: file_name.clone(),
                    error: e.to_string(),
                    failed_count: state.failed_count,
                }) {
                    warn!(error = %e, "Failed to emit ai-edit-progress Failed event");
                }

                if let Some(tx) = task.result_tx {
                    let _ = tx.send(Err(e.clone()));
                }
            }
            None => {
                // Task was cancelled during processing
                if let Some(tx) = task.result_tx {
                    let _ = tx.send(Err(AppError::AiEditError("AI edit cancelled".to_string())));
                }
                drain_pending_tasks(&mut manual_rx, &mut auto_rx, &queue_depth);
                emit_batch_done(&mut state, &app_handle);
                continue;
            }
        }

        // Emit Done when queue is empty and batch is complete
        if queue_depth.load(Ordering::Relaxed) == 0 && state.processed_count() > 0 {
            emit_batch_done(&mut state, &app_handle);
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
        .map_err(|e| AppError::AiEditError(format!("Preprocessing task panicked: {}", e)))?
        .map_err(|e| AppError::AiEditError(format!("Image preprocessing failed: {}", e)))?;

    let current_api_key = match &ai_config.provider {
        super::config::ProviderConfig::SeedEdit(cfg) => cfg.api_key.clone(),
    };
    // Use override_model for manual edits, fall back to config model for auto edits
    let effective_model = task.override_model.as_deref()
        .unwrap_or_else(|| match &ai_config.provider {
            super::config::ProviderConfig::SeedEdit(cfg) => cfg.model.as_str(),
        })
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
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn auto_trigger_sends_to_auto_channel() {
        let (auto_sender, mut auto_rx) = mpsc::channel::<AiEditTask>(AUTO_QUEUE_CAPACITY);
        let (_manual_sender, _manual_rx) = mpsc::channel::<AiEditTask>(MANUAL_QUEUE_CAPACITY);

        let task_path = PathBuf::from("/photos/test.jpg");
        auto_sender.try_send(AiEditTask {
            file_path: task_path.clone(),
            override_prompt: None,
            override_model: None,
            result_tx: None,
        }).unwrap();

        let task = auto_rx.try_recv().expect("task should be in auto channel");
        assert_eq!(task.file_path, task_path);
        assert!(task.result_tx.is_none());
    }

    #[tokio::test]
    async fn manual_trigger_sends_to_manual_channel() {
        let (_auto_sender, _auto_rx) = mpsc::channel::<AiEditTask>(AUTO_QUEUE_CAPACITY);
        let (manual_sender, mut manual_rx) = mpsc::channel::<AiEditTask>(MANUAL_QUEUE_CAPACITY);

        let (tx, _rx) = oneshot::channel();
        manual_sender.try_send(AiEditTask {
            file_path: PathBuf::from("/photos/test.jpg"),
            override_prompt: None,
            override_model: None,
            result_tx: Some(tx),
        }).unwrap();

        let task = manual_rx.try_recv().expect("task should be in manual channel");
        assert!(task.result_tx.is_some());
    }

    #[tokio::test]
    async fn auto_queue_full_drops_gracefully() {
        let (auto_sender, _auto_rx) = mpsc::channel::<AiEditTask>(AUTO_QUEUE_CAPACITY);
        let (_manual_sender, _manual_rx) = mpsc::channel::<AiEditTask>(MANUAL_QUEUE_CAPACITY);

        for i in 0..AUTO_QUEUE_CAPACITY {
            auto_sender.try_send(AiEditTask {
                file_path: PathBuf::from(format!("/photos/img_{i}.jpg")),
                override_prompt: None,
                override_model: None,
                result_tx: None,
            }).unwrap();
        }

        let result = auto_sender.try_send(AiEditTask {
            file_path: PathBuf::from("/photos/overflow.jpg"),
            override_prompt: None,
            override_model: None,
            result_tx: None,
        });
        assert!(result.is_err());
    }

    #[test]
    fn constants_are_reasonable() {
        assert_eq!(AUTO_QUEUE_CAPACITY, 32);
        assert_eq!(MANUAL_QUEUE_CAPACITY, 4);
        assert_eq!(AIEDIT_SUBDIR, "AIEdit");
    }

    #[test]
    fn chrono_now_string_includes_milliseconds() {
        let s = chrono_now_string();
        assert!(s.contains('.'), "Expected millisecond separator: got {}", s);
        let parts: Vec<&str> = s.split('.').collect();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[1].len(), 3, "Expected 3-digit milliseconds");
    }

    #[test]
    fn default_edit_prompt_is_chinese() {
        assert!(!_DEFAULT_EDIT_PROMPT.is_empty());
        assert!(_DEFAULT_EDIT_PROMPT.contains("画质"));
    }

    #[test]
    fn worker_state_tracks_output_files() {
        struct WorkerState {
            completed_count: u32,
            failed_count: u32,
            failed_files: Vec<String>,
            output_files: Vec<String>,
            batch_total: u32,
        }

        impl WorkerState {
            fn processed_count(&self) -> u32 {
                self.completed_count + self.failed_count
            }
        }

        let mut state = WorkerState {
            completed_count: 0,
            failed_count: 0,
            failed_files: Vec::new(),
            output_files: Vec::new(),
            batch_total: 0,
        };

        // Simulate successful task
        state.completed_count += 1;
        state.output_files.push("/output/AIEdit/photo1_AIEdit.jpg".to_string());
        assert_eq!(state.processed_count(), 1);
        assert_eq!(state.output_files.len(), 1);

        // Simulate another success
        state.completed_count += 1;
        state.output_files.push("/output/AIEdit/photo2_AIEdit.jpg".to_string());
        assert_eq!(state.processed_count(), 2);
        assert_eq!(state.output_files.len(), 2);

        // Simulate failure
        state.failed_count += 1;
        state.failed_files.push("bad.jpg".to_string());
        assert_eq!(state.processed_count(), 3);
        assert_eq!(state.output_files.len(), 2);
        assert_eq!(state.failed_files.len(), 1);
    }

    #[test]
    fn cancel_token_initially_not_cancelled() {
        let token = CancellationToken::new();
        assert!(!token.is_cancelled());
    }

    #[test]
    fn cancel_token_cancels_on_invoke() {
        let token = CancellationToken::new();
        token.cancel();
        assert!(token.is_cancelled());
    }

    #[tokio::test]
    async fn cancel_token_cancelled_future_resolves() {
        let token = CancellationToken::new();
        token.cancel();
        // Should resolve immediately
        token.cancelled().await;
    }
}
