# Event System Simplification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the current mixed replay/catch-up event bus with a simpler split between durable runtime state synchronization and transient event delivery, while preserving frontend UI updates, Android native service sync, and Windows tray behavior.

**Architecture:** Introduce a dedicated Rust runtime-state holder for replayable server status and stats, and keep transient domain events on a separate stream. Consumers that need the latest authoritative state attach to the state holder directly; transient consumers continue receiving only post-subscription events. This removes replay synthesis from the transient bus and makes startup correctness depend on explicit state reads instead of event reconstruction.

**Tech Stack:** Rust, Tokio, Tauri v2, Android JNI sync, Windows tray integration

---

### Task 1: Add failing tests that express the new split between state and transient delivery

**Files:**
- Modify: `src-tauri/src/ftp/events.rs`
- Modify: `src-tauri/src/ftp/types.rs`

- [ ] **Step 1: Write the failing test**

Replace or supplement the current replay/catch-up tests with explicit guarantees for the new design. Add tests in `src-tauri/src/ftp/events.rs` that describe:

```rust
#[tokio::test]
async fn late_state_subscriber_reads_current_snapshot_without_event_replay() {
    let runtime_state = ServerRuntimeState::default();
    runtime_state.update_running_snapshot(ServerStateSnapshot {
        is_running: true,
        connected_clients: 2,
        files_received: 7,
        bytes_received: 2048,
        last_file: None,
    });

    let snapshot = runtime_state.current_snapshot().await;

    assert!(snapshot.is_running);
    assert_eq!(snapshot.connected_clients, 2);
}

#[tokio::test]
async fn transient_subscriber_does_not_receive_pre_subscription_file_events() {
    let transient_bus = TransientEventBus::new();
    transient_bus.emit(DomainEvent::FileUploaded {
        path: "/before.jpg".into(),
        size: 512,
    });

    let mut rx = transient_bus.subscribe();

    assert!(rx.try_recv().is_err());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
./build.sh windows android
```

Expected: FAIL until the new state/transient split exists.

- [ ] **Step 3: Write minimal implementation**

Introduce type-level separation for runtime state vs transient stream, but do not rewire all consumers yet.

- [ ] **Step 4: Run test to verify it passes**

Run the same command and expect success.

### Task 2: Move durable server state ownership out of replay events

**Files:**
- Modify: `src-tauri/src/ftp/server.rs`
- Modify: `src-tauri/src/ftp/stats.rs`
- Modify: `src-tauri/src/ftp/types.rs`
- Modify: `src-tauri/src/ftp/events.rs`

- [ ] **Step 1: Write the failing test**

Add tests that show authoritative runtime state is readable directly without reconstructing it from event replay:

```rust
#[tokio::test]
async fn runtime_state_remains_coherent_across_start_stats_stop() {
    let runtime_state = ServerRuntimeState::default();

    runtime_state.record_server_started("192.168.1.8:2121".into()).await;
    runtime_state.record_stats(ServerStats::default().with_connected_clients(3)).await;
    runtime_state.record_server_stopped().await;

    let snapshot = runtime_state.current_snapshot().await;
    assert!(!snapshot.is_running);
    assert_eq!(snapshot.connected_clients, 0);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
./build.sh windows android
```

Expected: FAIL until durable state is no longer replay-derived.

- [ ] **Step 3: Write minimal implementation**

Create or refactor a `ServerRuntimeState` holder sourced from authoritative server + stats state and stop deriving latest state from replayed `DomainEvent`s.

- [ ] **Step 4: Run test to verify it passes**

Run the same command and expect success.

### Task 3: Rewire state consumers to read runtime state directly

**Files:**
- Modify: `src-tauri/src/ftp/events.rs`
- Modify: `src-tauri/src/ftp/server_factory.rs`
- Modify: `src-tauri/src/platform/android.rs`
- Modify: `src-tauri/src/platform/windows.rs`

- [ ] **Step 1: Write the failing test**

Add behavior-level tests that prove frontend state fan-out, Android native sync, and Windows tray/runtime consumers all observe the same authoritative state transitions without replay reconstruction.

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
./build.sh windows android
```

Expected: FAIL until handlers are fed from the new runtime-state path.

- [ ] **Step 3: Write minimal implementation**

Make state consumers subscribe/read from runtime state, and keep transient handlers on the transient event stream only.

- [ ] **Step 4: Run test to verify it passes**

Run the same command and expect success.

### Task 4: Remove replay/catch-up complexity from the transient bus

**Files:**
- Modify: `src-tauri/src/ftp/events.rs`
- Modify: `src-tauri/src/commands/server.rs`
- Modify: `src-tauri/src/file_index/service.rs`
- Modify: `src-tauri/src/ftp/listeners.rs`

- [ ] **Step 1: Write the failing test**

Add tests that prove transient delivery no longer depends on replay/catch-up and still preserves post-subscription file/index events.

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
./build.sh windows android
```

Expected: FAIL until the old mixed replay/catch-up logic is removed or simplified.

- [ ] **Step 3: Write minimal implementation**

Delete the obsolete replay-state synthesis, deferred stats staging, queued startup event alignment, and catch-up readiness semantics from the transient bus path.

- [ ] **Step 4: Run test to verify it passes**

Run the same command and expect success.

### Task 5: Final verification and regression check

**Files:**
- Modify: `docs/superpowers/specs/2026-03-28-event-system-simplification-design.md`
- Modify: `docs/superpowers/plans/2026-03-28-event-system-simplification.md`

- [ ] **Step 1: Run focused Rust verification**

Run:

```bash
./build.sh windows android
```

Expected: both targets still build and the Rust test suite covering event/state behavior passes.

- [ ] **Step 2: Run Android targeted tests**

Run:

```bash
./gradlew :app:testUniversalDebugUnitTest --tests "com.gjk.cameraftpcompanion.AndroidServiceStateCoordinatorTest" --tests "com.gjk.cameraftpcompanion.FtpForegroundServiceTest"
```

Expected: PASS.

- [ ] **Step 3: Run frontend targeted tests**

Run:

```bash
npm test -- server-events serverStore.characterization
```

Expected: PASS.

- [ ] **Step 4: Confirm simplification goals in diff review**

Verify the final diff materially removes replay/catch-up complexity from `src-tauri/src/ftp/events.rs` and keeps state/transient responsibilities separated.
