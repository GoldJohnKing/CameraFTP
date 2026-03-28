# Event System Simplification Design

**Date:** 2026-03-28

**Goal:** Simplify the Rust event system so startup synchronization, replay, and cross-consumer fan-out are easier to reason about, while preserving correct delivery to both frontend UI consumers and Android native service consumers.

---

## Problem Statement

Direction 2 made Rust the single source of truth and added dual fan-out to frontend UI and Android native service state. To close startup gaps, the current event system grew replay, catch-up, deferred stats, queued non-state events, and readiness semantics inside one mixed event bus.

That design now works, but it has drawbacks:

- startup ordering is hard to reason about,
- state and non-state events have different delivery guarantees but share one mechanism,
- replay/catch-up logic is more complex than the actual business events,
- new regressions are likely when adding more handlers or lifecycle hooks.

We want a simpler model with explicit delivery semantics.

---

## Scope

### In scope

- Redesign Rust-side event delivery boundaries.
- Separate state synchronization from transient event delivery.
- Preserve Android direct-native service sync and frontend UI updates.
- Reduce startup replay/catch-up complexity.
- Keep Windows behavior unchanged.

### Out of scope

- Redesigning frontend store shape.
- Replacing Tauri events entirely.
- Changing Android notification semantics.
- Reworking FTP domain event meaning.

---

## Chosen Approach

Split the current mixed event system into two explicit channels:

1. **State channel**
   - Carries the latest runtime server state snapshot.
   - Consumers read the current snapshot immediately when attaching.
   - No replay queue, no deferred event synthesis.

2. **Transient event channel**
   - Carries one-off domain events such as `FileUploaded` and `FileIndexChanged`.
   - No replay guarantee for pre-subscription history.
   - Only events emitted after subscription are delivered.

This removes the need to make one mechanism satisfy both state synchronization and event-stream semantics.

---

## Alternatives Considered

### 1. Keep current mixed event bus and keep patching edge cases

**Rejected** because complexity will keep increasing as more startup or lifecycle bugs are discovered.

### 2. Split state and transient channels (chosen)

**Accepted** because it aligns implementation with the actual guarantees consumers need.

### 3. Push all synchronization responsibility into each handler

**Rejected** because each handler would reimplement subscription, replay, and startup synchronization differently.

---

## Architecture

### State model

Introduce a dedicated runtime state holder in Rust, for example `ServerRuntimeState`, containing the latest coherent snapshot needed by long-lived consumers.

Suggested contents:

- running / stopped state,
- current server info (usable advertised address, port),
- latest aggregated stats snapshot,
- connected client count,
- optional last file metadata if already part of the canonical snapshot.

Consumers do not reconstruct startup state from old events. They attach and read the current snapshot directly.

### State subscribers

The following become state subscribers:

- frontend stats/UI fan-out,
- Android direct-native service sync,
- Windows tray/server-status updater if it depends on runtime state.

### Transient domain events

Keep a separate event stream for:

- `FileUploaded`,
- `FileIndexChanged`,
- other future fire-and-forget notifications.

These events are not replayed to late subscribers.

---

## Data Flow

### Server start

1. FTP server start succeeds.
2. Rust updates `ServerRuntimeState`.
3. State subscribers observe the new running snapshot.
4. UI and Android native service both update from the same state.

### Stats update

1. Rust updates the latest stats in `ServerRuntimeState`.
2. State subscribers receive the new snapshot.
3. No separate replay logic is needed for later subscribers.

### Server stop

1. Rust resets `ServerRuntimeState` to a stopped snapshot.
2. State subscribers observe the stopped state.
3. Android coordinator stops the foreground service; frontend resets UI state.

### File upload

1. Domain emits `FileUploaded`.
2. Transient event subscribers receive it if currently subscribed.
3. No replay occurs for past file events.

---

## Simplification Targets

The new model should eliminate or drastically shrink:

- replay-state synthesis from old domain events,
- deferred stats staging,
- queued startup event buffers for state alignment,
- catch-up drains that must distinguish state vs non-state delivery,
- readiness signals that mean “subscription plus replay plus queue reconciliation”.

State attachment should become: **subscribe → read current snapshot → receive future updates**.

---

## Consistency Rules

- Rust runtime state remains the single source of truth.
- UI and Android service state consumers read from the same latest snapshot.
- Transient events never mutate the authoritative state by themselves.
- State consumers must tolerate repeated identical snapshots or use explicit deduping at the subscriber boundary.
- Startup correctness must no longer depend on replaying old domain events in the right order.

---

## Testing Strategy

### Rust tests

Add focused tests for:

- late state subscriber receives the current snapshot immediately,
- transient subscribers do not receive pre-subscription file events,
- state updates remain coherent across start → stats → stop,
- Android native sync and frontend state subscribers both observe the same transitions.

### Integration checks

- existing Windows tray/runtime behavior still updates correctly,
- Android direct-native service state still updates correctly on start/update/stop,
- frontend UI state remains unchanged from the user perspective.

---

## Success Criteria

This simplification is complete when:

1. state synchronization no longer depends on replaying startup events,
2. state and transient event delivery are implemented through distinct mechanisms,
3. startup ordering code becomes materially smaller and easier to understand,
4. Android native service sync and frontend UI updates still come from the same Rust state,
5. Windows behavior remains unchanged,
6. targeted tests prove the new guarantees explicitly.
