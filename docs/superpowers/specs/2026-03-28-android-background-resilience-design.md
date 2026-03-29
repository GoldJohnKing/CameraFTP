# Android Background Resilience Design

**Date:** 2026-03-28

**Goal:** Strengthen Android background reliability so the FTP service and foreground notification remain correct across Activity destruction, service recreation, process churn, package replacement, and other long-running lifecycle disruptions.

---

## Problem Statement

The current Android architecture is much stronger than before:

- WebView no longer controls foreground service state,
- Rust directly syncs Android native service state,
- coordinator and service restore from native snapshot state,
- major crash paths have been removed.

However, long-running Android background reliability is still limited by process-local state and manual adb validation. If the app process is killed or the service is recreated under different timing conditions, we need stronger guarantees that service state, notification state, and recovery behavior stay aligned with the real server lifecycle.

---

## Scope

### In scope

- Improve Android service resilience across lifecycle disruptions.
- Define what state must survive beyond the current process.
- Add better recovery behavior and diagnostics.
- Add automated adb/runtime verification for background residency.

### Out of scope

- Guaranteeing survival against user force-stop or all OEM kill policies.
- Moving the FTP server implementation into Android native code.
- Replacing the Rust single-source-of-truth design.

---

## Chosen Approach

Add a **persistent Android recovery layer** around the existing native coordinator/service path:

1. Persist the minimum service-recovery snapshot outside the process.
2. Make service recreation explicitly reconcile persisted Android state with current Rust-reported runtime state.
3. Add structured diagnostics and automated runtime verification.

This keeps the current architecture but makes it more resilient to real Android lifecycle behavior.

---

## Alternatives Considered

### 1. Keep current in-memory coordinator only

**Rejected** because process death or rebuild scenarios still lose valuable recovery context.

### 2. Add persistent recovery layer around current design (chosen)

**Accepted** because it strengthens reliability without re-architecting ownership.

### 3. Move full server ownership into Android native service

**Rejected for now** because it would create a larger cross-platform split and duplicate Rust responsibilities.

---

## Architecture

### Recovery snapshot

Persist a minimal snapshot using Android-native storage such as `SharedPreferences`.

Suggested fields:

- `isRunning`,
- latest stats snapshot,
- latest connected client count,
- latest known server info if needed for notification display,
- `lastTransitionAt`,
- `lastTransitionSource` or `lastTransitionReason` for diagnostics.

This snapshot is not a new source of truth. It is a recovery aid used until fresh Rust state arrives.

### Coordinator role

`AndroidServiceStateCoordinator` remains the in-process state/control hub, but it also:

- writes persisted recovery state on transitions,
- restores persisted recovery state during startup or recreation,
- invalidates persisted running state when a confirmed stop occurs.

### Service role

`FtpForegroundService` remains the owner of foreground notification presentation. On recreation it should:

1. read coordinator state,
2. if empty, read persisted recovery state,
3. start in the safest accurate mode available,
4. reconcile with fresh Rust/native sync when it arrives.

---

## Recovery Matrix

### Activity destroyed, process alive

- No special recovery needed.
- Current coordinator/service path should continue working unchanged.

### Service recreated, process alive

- Service restores from coordinator snapshot immediately.
- Notification remains correct.

### Process killed, app later relaunched

- Coordinator restores from persisted recovery snapshot.
- Service starts from that snapshot only as an interim recovery state.
- Fresh Rust sync must reconcile the state quickly.

### Package replaced / app updated

- Persisted recovery snapshot may survive installation.
- On first launch after replace, app must revalidate persisted “running” state before treating it as authoritative.

### Rust reports stopped after stale persisted running snapshot

- Stop transition clears both in-memory and persisted running state.
- Notification/service must be shut down deterministically.

---

## Reliability Safeguards

### Explicit stale-state invalidation

Persisted state must include enough metadata to detect obviously stale recovery state, such as:

- timestamp age,
- app version/build marker,
- whether a clean stop was observed.

### Recovery precedence

Use this priority:

1. fresh Rust/native sync,
2. in-memory coordinator snapshot,
3. persisted recovery snapshot.

Persisted state is fallback only.

### Defensive service startup

If recovery data is incomplete or inconsistent, prefer a conservative notification state and wait for fresh Rust reconciliation rather than claiming the wrong state confidently.

---

## Diagnostics

Add structured logging around:

- Rust → Android native sync attempts,
- coordinator state writes and restores,
- service start/update/stop reasons,
- stale-start rejections,
- persisted-state restoration and invalidation decisions.

This should make future background failures diagnosable without broad guesswork.

---

## Automated Verification

Create adb-based runtime verification covering:

1. install latest APK,
2. launch app,
3. start FTP server,
4. background app,
5. verify process + notification,
6. optionally simulate activity destruction / app relaunch,
7. verify notification/state remain correct,
8. scan logs for fatal signatures.

Over time, extend this with scenarios such as package replace or service recreation if feasible.

---

## Testing Strategy

### Android unit tests

Add tests for:

- persisted snapshot write/read behavior,
- coordinator restore precedence,
- stale snapshot invalidation,
- service behavior when only persisted recovery state exists.

### Runtime validation

Automate adb scripts for the supported background-residency scenarios.

### Cross-platform verification

- Windows must remain unaffected.
- Rust state ownership must remain unchanged.

---

## Success Criteria

This resilience work is complete when:

1. Android service state can recover correctly from more than just in-process state,
2. persisted recovery state is clearly secondary to fresh Rust state,
3. stale recovery state is detected and invalidated safely,
4. service and notification recovery paths are logged and testable,
5. automated adb verification covers the main background residency scenarios,
6. Windows behavior remains unchanged.
