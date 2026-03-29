# Android Service Residency Hardening Design

**Date:** 2026-03-27

**Goal:** Stabilize the existing Android foreground-service residency chain so the FTP service notification, wake locks, and background presence remain correct while the app process lives, and prepare the codebase for a later full decoupling from `MainActivity`/WebView.

---

## Problem Statement

The current Android service-control path is UI-mediated:

1. Rust emits server lifecycle/stat events.
2. Frontend event listeners in the WebView receive them.
3. JavaScript calls `ServerStateAndroid.onServerStateChanged(...)`.
4. `ServerStateBridge` forwards to `MainActivity.updateServiceState(...)`.
5. `MainActivity` starts, updates, or stops `FtpForegroundService`.

This works only while the WebView and `MainActivity` are healthy. It has several concrete weaknesses:

- `getOrCreateService()` can return `null` immediately after requesting service startup, dropping the first update.
- `FtpForegroundService` keeps notification state only in memory, so sticky recreation loses state until a later UI resync happens.
- Service control depends on `MainActivity`, even though the product goal is a long-running background FTP service.
- The current bridge path is also the wrong long-term abstraction boundary for direction 2, where service state should become native-owned instead of UI-relayed.

---

## Scope

### In scope for direction 1

- Harden the current service startup/update/stop chain.
- Introduce an Android-native latest-state snapshot that survives Activity/WebView loss inside the same app process and can be restored by the service on recreation.
- Remove `MainActivity` as the only in-process source of truth for foreground-service state transitions.
- Keep the current frontend-driven sync path working.
- Prepare a clean seam so direction 2 can later feed the same native service-state coordinator directly from Android/Rust integration.

### Out of scope for direction 1

- Full architectural decoupling of service control from the WebView event system.
- Moving FTP server ownership into Android native service code.
- Cross-process persistence after full process death and cold restart.
- Redesigning user-facing notification content or Android permission UX.

---

## Chosen Approach

Use a new **Android service-state coordinator** as the in-process native state holder and control plane for foreground-service state.

The coordinator will:

- store the latest service snapshot (`isRunning`, stats JSON, connected clients),
- expose explicit `update(snapshot)` and `clear()` operations,
- start or stop `FtpForegroundService` using application context instead of Activity instance timing,
- let `FtpForegroundService` pull the latest snapshot during `onCreate()` / `onStartCommand()`,
- allow future direction-2 callers to reuse the same coordinator without going through `MainActivity`.

This keeps the current architecture mostly intact but removes the most fragile dependency edges.

---

## Alternatives Considered

### 1. Minimal patch only

Patch `getOrCreateService()` with retries or delays and keep all state in `MainActivity`.

**Rejected** because it fixes the race but does not fix state restoration or provide any reusable seam for direction 2.

### 2. Full direction-2 decoupling now

Move all service-state sync to Android native and make the WebView UI-only.

**Rejected for now** because it is a larger architectural change than needed to stabilize the current release.

### 3. Coordinator-based hardening (chosen)

Adds a native snapshot/control layer now, while preserving current frontend inputs.

**Accepted** because it fixes immediate bugs and is the best preparation for direction 2.

---

## Architecture

### New native responsibility split

#### `AndroidServiceStateCoordinator` (new)

Responsibility:

- hold the latest known service snapshot in a process-wide native location,
- own app-context service start/stop/update requests,
- act as the boundary between callers and `FtpForegroundService`.

Expected API shape:

- `updateServiceState(context, isRunning, statsJson, connectedClients)`
- `getLatestState()`
- `clearState()`
- internal `startForegroundServiceIfNeeded(context)`
- internal `stopForegroundService(context)` when state transitions to stopped

This is the direction-2 seam.

#### `FtpForegroundService`

Responsibility:

- remain the owner of the actual foreground notification, wake lock, and Wi‑Fi lock,
- render notification content from the latest snapshot,
- restore notification state from the coordinator if it is recreated.

It should no longer depend on `MainActivity` timing to become correct.

#### `ServerStateBridge`

Responsibility:

- remain the current WebView entrypoint for direction 1,
- forward directly to the coordinator instead of `MainActivity.updateServiceState(...)`.

This shrinks `MainActivity`’s role and prevents service state from being tied to an Activity instance.

#### `MainActivity`

Responsibility after direction 1:

- manage UI and WebView lifecycle,
- no longer serve as the sole service-state relay,
- possibly retain only helper functions that are still UI-specific.

---

## Data Model

Introduce a small Android-native snapshot type.

Suggested fields:

- `isRunning: Boolean`
- `statsJson: String?`
- `connectedClients: Int`

The coordinator stores exactly one latest snapshot.

Rules:

- `isRunning=false` means the foreground service should stop and native cached state should reset to a default stopped snapshot.
- `isRunning=true` means the latest stats should be retained even if service recreation happens before another frontend event arrives.
- `statsJson` stays as JSON string for now to minimize scope and avoid duplicating frontend/Rust snapshot parsing logic during direction 1.

---

## Data Flow

### Running-state update

1. Frontend receives `server-started` or `stats-update`.
2. Frontend calls existing Android sync function.
3. JS bridge calls `ServerStateBridge.onServerStateChanged(...)`.
4. `ServerStateBridge` forwards to `AndroidServiceStateCoordinator.updateServiceState(...)`.
5. Coordinator stores latest snapshot.
6. Coordinator ensures `FtpForegroundService` is running.
7. `FtpForegroundService` reads latest snapshot and updates its notification.

### Stop-state update

1. Frontend receives `server-stopped`.
2. Bridge forwards stopped state to coordinator.
3. Coordinator clears snapshot and requests service stop.
4. Service releases locks and exits.

### Sticky service recreation

1. Android recreates `FtpForegroundService`.
2. Service reads latest snapshot from coordinator.
3. If snapshot says running, service immediately rebuilds foreground notification from it.
4. If snapshot says stopped, service exits or remains stopped.

---

## Failure Handling

### Service start race

The coordinator must not rely on `FtpForegroundService.getInstance()` immediately becoming non-null. It should issue the start request using app context and treat the latest snapshot as authoritative. Service startup becomes eventual, not synchronous.

### Missing stats on first event

If only `isRunning=true` is known at first, service should still start with a valid “running” notification using default zeroed counters until a later stats update arrives.

### Activity destruction

Activity/WebView destruction must not clear or mutate coordinator state. UI teardown is independent from service state.

### Future direct-native callers

The coordinator interface must not require an Activity instance. It should accept `Context` or use application context only.

---

## Testing Strategy

### Unit tests

Add Android unit tests to verify:

- updating the coordinator with `isRunning=true` stores the latest snapshot,
- clearing/stopping resets snapshot,
- service recreation reads previously stored snapshot,
- bridge calls no longer require `MainActivity.updateServiceState(...)`,
- default notification content remains valid with partial stats.

### Regression coverage

Add a failing test first for the startup-race class of bug: a caller can update running state before `FtpForegroundService.getInstance()` exists, and the desired state must still be preserved for later service creation.

### Full verification

- focused Android unit tests,
- required project build: `./build.sh windows android`,
- adb validation that server start still produces ongoing notification and that backgrounding the app does not clear service state while the process stays alive.

---

## Direction 2 Preparation

Direction 1 intentionally introduces the seam direction 2 needs.

After this work, the next step can replace the WebView bridge as the primary writer into `AndroidServiceStateCoordinator` with a direct Android-native integration path from Rust/Tauri. The service and notification path will already consume state from the coordinator, so direction 2 becomes a control-plane migration instead of another service rewrite.

Planned direction-2 leverage points:

- `AndroidServiceStateCoordinator` becomes the stable native API,
- `ServerStateBridge` becomes optional or UI-only,
- `MainActivity` no longer matters for background service correctness.

---

## Success Criteria

Direction 1 is complete when all of the following are true:

1. A foreground-service state update no longer depends on `FtpForegroundService.getInstance()` being immediately available.
2. `FtpForegroundService` can restore its visible notification state from native snapshot state after recreation.
3. `ServerStateBridge` no longer routes through `MainActivity.updateServiceState(...)`.
4. Existing frontend-driven Android sync still works without changing product behavior.
5. Tests and full builds pass.
6. The resulting coordinator API is suitable for direction 2 to call directly later.
