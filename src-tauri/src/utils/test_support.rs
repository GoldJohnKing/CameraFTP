// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Shared helpers for the AI-edit and color-grading worker-loop tests,
//! extracted from the near-identical copies that used to live in both
//! services' test modules. Compiled only under `cfg(test)` (see the
//! `#[cfg(test)]` declaration in `utils/mod.rs`).

use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::de::DeserializeOwned;
use tauri::Listener;

/// Collects deserialized `event` payloads of type `E` from the mock app.
///
/// Generic over the event enum so one implementation serves both
/// `ai-edit-progress` (`AiEditProgressEvent`) and
/// `color-grading-progress` (`ColorGradingEvent`).
pub(crate) fn event_collector<E>(
    handle: &tauri::AppHandle<tauri::test::MockRuntime>,
    event: &str,
) -> Arc<Mutex<Vec<E>>>
where
    E: DeserializeOwned + Send + 'static,
{
    let events: Arc<Mutex<Vec<E>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&events);
    handle.listen(event, move |e| {
        if let Ok(ev) = serde_json::from_str::<E>(e.payload()) {
            sink.lock().unwrap().push(ev);
        }
    });
    events
}

/// Polls `probe` until it returns `Some` or the timeout elapses (panics).
pub(crate) async fn wait_until<T>(timeout: Duration, mut probe: impl FnMut() -> Option<T>) -> T {
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
