# Android Direct Native Service Sync Design

**Date:** 2026-03-27

**Goal:** Remove Android foreground-service control's dependency on WebView/JavaScript while keeping UI state updates working cross-platform, so both Android service state and frontend UI state are updated from the same Rust state source and remain consistent.

---

## Problem Statement

Direction 1 introduced `AndroidServiceStateCoordinator` and moved Android foreground-service state handling out of `MainActivity`, but Android service updates are still ultimately initiated from frontend-driven sync. That means Android service correctness still depends on the WebView booting and JavaScript bridge availability, even though the product requirement is that the Android foreground service must remain correct independently of the WebView.

We need direction 2 to make Android service state native-driven while preserving the existing cross-platform UI event flow.

---

## Constraints

- This is a cross-platform app (Windows + Android).
- Windows behavior must not regress.
- Tauri events must remain available for frontend UI/store updates on all platforms.
- Android foreground-service state must stop depending on WebView, JS bridge, or frontend store timing.
- UI state and Android service state must still converge to the same final result.

---

## Chosen Approach

Use a **single Rust state source with dual fan-out**:

1. **UI chain** remains unchanged in principle:
   - Rust server lifecycle/state → Tauri events → frontend store/UI
2. **Android native service chain** becomes direct:
   - Rust server lifecycle/state → Rust platform abstraction → Android platform implementation → Kotlin `AndroidServiceStateCoordinator` → `FtpForegroundService`

This makes Rust the single state authority, while the frontend and Android native service each consume state independently.

---

## Alternatives Considered

### 1. Keep frontend as the Android service relay

**Rejected** because it leaves WebView dependency in place.

### 2. Rust direct Android sync through platform abstraction (chosen)

**Accepted** because it removes WebView dependency without changing Windows semantics when properly isolated behind platform interfaces.

### 3. Android native polling Rust state

**Rejected** because polling introduces lag, extra complexity, and weaker consistency than event-driven fan-out.

---

## Architecture

### Single source of truth

Shared Rust server lifecycle/state remains authoritative for:

- server started,
- server stopped,
- stats updates,
- connected-client count.

Neither frontend store nor Android native code may become a second writer for service state.

### UI chain

Rust continues emitting the existing Tauri events used by frontend code. This keeps Windows and Android UI behavior aligned and avoids introducing a new UI-specific Android path.

### Android native service chain

Rust platform abstraction is extended so Android platform code can receive native service-state updates directly. The Android implementation forwards those updates into native Kotlin coordinator/service code. Windows implementation remains no-op.

### Migration strategy

This direction uses a **gradual cutover**:

1. Add the Rust→Android native path.
2. Keep frontend-driven Android sync temporarily, but downgrade it so it no longer owns service control.
3. Verify the new native path works.
4. Remove the old frontend Android service-control path entirely.

This reduces migration risk while preserving product behavior during rollout.

---

## Boundaries

### Shared Rust code

Shared Rust code may decide *when* Android service state should change, but it must not contain raw Android JNI details or Android-specific lifecycle code.

### Platform abstraction

All Android-specific native sync must stay behind the existing Rust platform boundary.

Required shape:

- shared trait method for service-state sync,
- Android implementation performs native dispatch,
- Windows implementation is no-op.

### Kotlin Android side

Android native side remains responsible for:

- latest service snapshot,
- service start/update/stop,
- notification rendering,
- stale-start protection,
- service recreation restore.

### Frontend

Frontend continues owning UI state only. It must no longer be the control plane for Android service state.

---

## Data Flow

### Server started

1. Rust starts the FTP server.
2. Rust emits existing Tauri UI events.
3. Rust also invokes Android platform service-state sync with running snapshot.
4. Android native coordinator starts/updates `FtpForegroundService`.
5. Frontend UI and Android notification both reflect the same Rust-originated running state.

### Stats updated

1. Rust receives stats update.
2. Rust emits the existing frontend stats event.
3. Rust invokes Android platform service-state sync with updated counters/clients.
4. Android foreground notification updates independently of WebView.

### Server stopped

1. Rust stops the FTP server.
2. Rust emits existing stopped event for UI.
3. Rust invokes Android platform service-state sync with stopped snapshot.
4. Android native coordinator clears snapshot and stops the service.

---

## Consistency Rules

- Rust is the only source of state transitions.
- Android foreground-service state must not depend on frontend store writes.
- Frontend UI continues to update from Rust events.
- Final UI state and final Android service state must match because both come from the same Rust state transition.
- During gradual cutover, duplicate writers must be prevented; temporary compatibility code must not compete with the native path.

---

## Windows Impact

Expected Windows impact is minimal if the new path remains behind the Rust platform abstraction.

Windows must continue to:

- receive the same Tauri UI events,
- avoid any Android-specific native sync work,
- compile without Android-only dependencies leaking into shared code.

The Windows platform implementation should explicitly do nothing for the new Android-service sync callback.

---

## Testing Strategy

### Rust/platform tests

Add tests around the platform abstraction or surrounding logic to verify Android service sync is invoked from Rust state transitions without altering existing frontend event behavior.

### Android native tests

Verify coordinator/service still handle:

- start,
- stats update,
- stop,
- stale start protection,
- restore after recreation,

when updates originate from the direct native path.

### Frontend tests

Update tests so Android service control is no longer asserted through frontend sync. Frontend tests should continue validating UI/store behavior only.

### Cross-platform verification

- Windows build passes unchanged.
- Android build passes.
- Android adb validation confirms foreground notification remains correct even if WebView is absent or backgrounded.

---

## Success Criteria

Direction 2 is complete when all of the following are true:

1. Android foreground-service control no longer depends on WebView or JavaScript bridge availability.
2. Rust directly fans out state to both frontend UI events and Android native service sync.
3. Windows behavior remains unchanged.
4. Frontend no longer acts as the Android service control plane.
5. Android notification/service state and frontend UI state both converge from the same Rust state transitions.
