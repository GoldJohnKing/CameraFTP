// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Small shared primitives for the AI-edit and color-grading worker
//! pipelines, extracted so both services keep an identical contract instead
//! of duplicating it in comments and atomic bookkeeping.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use tokio_util::sync::CancellationToken;

/// Cancel/re-arm gate for a worker's current batch.
///
/// The service and its worker share one gate. `cancel_and_rearm()` aborts
/// in-flight and queued work via the active token, then replaces it with a
/// new uncancelled token. Safe to call repeatedly — redundant calls are
/// silently absorbed because the worker only reacts to the first active
/// cancellation. Tasks enqueued after the call observe the fresh token via
/// `current()`.
#[derive(Clone)]
pub(crate) struct CancelGate {
    token: Arc<Mutex<CancellationToken>>,
}

impl Default for CancelGate {
    fn default() -> Self {
        Self::new()
    }
}

impl CancelGate {
    pub(crate) fn new() -> Self {
        Self {
            token: Arc::new(Mutex::new(CancellationToken::new())),
        }
    }

    /// Snapshot the currently armed token. Workers re-snapshot at the top of
    /// each loop iteration so a re-arm takes effect for subsequent tasks
    /// without restarting the worker.
    pub(crate) fn current(&self) -> CancellationToken {
        self.lock().clone()
    }

    /// Cancel the active token and arm a fresh uncancelled one for future
    /// tasks. See the type-level doc for the full contract.
    pub(crate) fn cancel_and_rearm(&self) {
        let mut guard = self.lock();
        guard.cancel();
        *guard = CancellationToken::new();
    }

    /// A poisoned lock means a panic raced a cancel; the token itself is
    /// still usable, so recover the guard instead of propagating the panic.
    fn lock(&self) -> MutexGuard<'_, CancellationToken> {
        self.token.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Shared queue-depth counter for a worker pipeline.
///
/// Enqueue side: `add(n)` BEFORE sending; if a send fails, roll the failed
/// portion back with `sub(n)` so the counter never counts unsent tasks. The
/// worker decrements as it dequeues/processes each task, so the counter
/// drains to exactly 0 when the queue is empty — that zero is what triggers
/// the batch-done event.
#[derive(Clone)]
pub(crate) struct QueueDepth {
    depth: Arc<AtomicU32>,
}

impl Default for QueueDepth {
    fn default() -> Self {
        Self::new()
    }
}

impl QueueDepth {
    pub(crate) fn new() -> Self {
        Self {
            depth: Arc::new(AtomicU32::new(0)),
        }
    }

    /// Enqueue-side bump (also used by tests mirroring the enqueue contract).
    pub(crate) fn add(&self, n: u32) {
        self.depth.fetch_add(n, Ordering::Relaxed);
    }

    /// Rollback for a failed send, or the worker-side per-task decrement.
    /// Always paired with a prior `add`.
    pub(crate) fn sub(&self, n: u32) {
        self.depth.fetch_sub(n, Ordering::Relaxed);
    }

    /// Zero the counter when respawning a dead worker (a fresh channel starts
    /// empty; leftovers from the dead worker were never going to be counted).
    pub(crate) fn reset(&self) {
        self.depth.store(0, Ordering::Relaxed);
    }

    pub(crate) fn get(&self) -> u32 {
        self.depth.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cancel_gate_current_is_initially_live() {
        let gate = CancelGate::new();
        assert!(!gate.current().is_cancelled());
    }

    #[tokio::test]
    async fn cancel_and_rearm_cancels_old_snapshot_and_arms_fresh_token() {
        let gate = CancelGate::new();
        let armed = gate.current();

        gate.cancel_and_rearm();

        assert!(armed.is_cancelled(), "snapshots taken before re-arm must fire");
        let fresh = gate.current();
        assert!(!fresh.is_cancelled(), "snapshots taken after re-arm must be live");
    }

    #[tokio::test]
    async fn cancel_and_rearm_is_idempotent_for_callers() {
        let gate = CancelGate::new();
        gate.cancel_and_rearm();
        gate.cancel_and_rearm();

        assert!(!gate.current().is_cancelled());
    }

    #[tokio::test]
    async fn gate_clones_share_the_same_token_slot() {
        let gate = CancelGate::new();
        let clone = gate.clone();
        let armed = gate.current();

        clone.cancel_and_rearm();

        assert!(armed.is_cancelled());
        assert!(!gate.current().is_cancelled());
    }

    #[test]
    fn queue_depth_add_sub_pairs_drain_to_zero() {
        let depth = QueueDepth::new();

        depth.add(3);
        assert_eq!(depth.get(), 3);
        depth.sub(1);
        depth.sub(2);
        assert_eq!(depth.get(), 0, "fully paired add/sub must drain to 0");
    }

    #[test]
    fn queue_depth_rollback_restores_prior_value() {
        let depth = QueueDepth::new();
        depth.add(2); // earlier successful enqueues

        // Failed send of a batch of 3 that only got 1 out → roll back the rest.
        depth.add(3);
        depth.sub(2);
        assert_eq!(depth.get(), 3);
    }

    #[test]
    fn queue_depth_reset_zeroes_for_worker_respawn() {
        let depth = QueueDepth::new();
        depth.add(7);
        depth.reset();
        assert_eq!(depth.get(), 0);
    }

    #[test]
    fn queue_depth_clones_share_the_same_counter() {
        let depth = QueueDepth::new();
        let clone = depth.clone();
        depth.add(5);
        assert_eq!(clone.get(), 5);
    }
}
