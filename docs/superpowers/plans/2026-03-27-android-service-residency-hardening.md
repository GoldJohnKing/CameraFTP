# Android Service Residency Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Harden Android foreground-service state handling so FTP service state survives Activity/WebView loss within the running app process and service startup no longer depends on immediate singleton availability.

**Architecture:** Add a native `AndroidServiceStateCoordinator` that owns the latest running/stats/client snapshot and start/stop intent dispatch. Keep the existing frontend bridge contract for now, but route it through the coordinator so `FtpForegroundService` can restore notification state on recreation and direction 2 can later plug into the same seam.

**Tech Stack:** Kotlin, Android foreground service, Robolectric, TypeScript frontend event sync

---

### Task 1: Add failing Android tests for native service-state coordination

**Files:**
- Create: `src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/AndroidServiceStateCoordinatorTest.kt`
- Create: `src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/bridges/ServerStateBridgeTest.kt`
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/ServerStateBridge.kt`

- [ ] **Step 1: Write the failing test**

Add tests that prove:

```kotlin
@Test
fun update_service_state_persists_snapshot_before_service_instance_exists() {
    val context = ApplicationProvider.getApplicationContext<Context>()

    AndroidServiceStateCoordinator.clearState()
    AndroidServiceStateCoordinator.updateServiceState(context, true, "{\"files_transferred\":1}", 2)

    val snapshot = AndroidServiceStateCoordinator.getLatestState()
    assertTrue(snapshot.isRunning)
    assertEquals(2, snapshot.connectedClients)
    assertEquals("{\"files_transferred\":1}", snapshot.statsJson)
}

@Test
fun bridge_forwards_to_coordinator_without_main_activity() {
    val context = ApplicationProvider.getApplicationContext<Context>()
    val bridge = ServerStateBridge(context)

    AndroidServiceStateCoordinator.clearState()
    bridge.onServerStateChanged(true, "{\"files_transferred\":3}", 1)

    val snapshot = AndroidServiceStateCoordinator.getLatestState()
    assertTrue(snapshot.isRunning)
    assertEquals(1, snapshot.connectedClients)
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
./gradlew :app:testUniversalDebugUnitTest --tests "com.gjk.cameraftpcompanion.AndroidServiceStateCoordinatorTest" --tests "com.gjk.cameraftpcompanion.bridges.ServerStateBridgeTest"
```

Expected: FAIL because the coordinator class does not exist and `ServerStateBridge` still requires `MainActivity`.

- [ ] **Step 3: Write minimal implementation**

Create a coordinator API and update the bridge constructor/forwarding path to use it.

- [ ] **Step 4: Run test to verify it passes**

Run the same command and expect PASS.

### Task 2: Make `FtpForegroundService` restore and use coordinator state

**Files:**
- Create: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/AndroidServiceStateCoordinator.kt`
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/FtpForegroundService.kt`
- Modify: `src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/AndroidServiceStateCoordinatorTest.kt`

- [ ] **Step 1: Write the failing test**

Extend coordinator tests with service-state restoration behavior:

```kotlin
@Test
fun service_restores_notification_state_from_coordinator_snapshot() {
    val context = ApplicationProvider.getApplicationContext<Context>()
    AndroidServiceStateCoordinator.updateServiceState(
        context,
        true,
        "{\"files_transferred\":5,\"bytes_transferred\":1024}",
        4,
    )

    val service = Robolectric.buildService(FtpForegroundService::class.java).create().get()
    service.onStartCommand(Intent(context, FtpForegroundService::class.java), 0, 1)

    val restored = AndroidServiceStateCoordinator.getLatestState()
    assertTrue(restored.isRunning)
    assertEquals(4, restored.connectedClients)
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
./gradlew :app:testUniversalDebugUnitTest --tests "com.gjk.cameraftpcompanion.AndroidServiceStateCoordinatorTest"
```

Expected: FAIL because the service does not yet read coordinator state.

- [ ] **Step 3: Write minimal implementation**

Implement:

- coordinator snapshot data class and storage,
- `FtpForegroundService` restore path from coordinator in `onCreate()` / `onStartCommand()`,
- idempotent `updateServerState(...)` backed by coordinator state.

- [ ] **Step 4: Run test to verify it passes**

Run the same command and expect PASS.

### Task 3: Remove `MainActivity` from the service-state relay path

**Files:**
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/MainActivity.kt`
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/ServerStateBridge.kt`
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/AndroidServiceStateCoordinator.kt`

- [ ] **Step 1: Write the failing test**

Add a test asserting `MainActivity.updateServiceState(...)` is no longer needed by bridge-driven sync and that service control works from application context.

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
./gradlew :app:testUniversalDebugUnitTest --tests "com.gjk.cameraftpcompanion.bridges.ServerStateBridgeTest"
```

Expected: FAIL until the bridge no longer depends on `MainActivity`.

- [ ] **Step 3: Write minimal implementation**

Keep `MainActivity` UI-focused. Remove or stop using `getOrCreateService()` from the bridge path, and route start/update/stop exclusively through the coordinator.

- [ ] **Step 4: Run test to verify it passes**

Run the same command and expect PASS.

### Task 4: Tighten frontend Android sync behavior without changing product behavior

**Files:**
- Modify: `src/services/android-server-state-sync.ts`
- Modify: `src/services/server-events.ts`
- Modify: `src/stores/serverStore.ts`
- Test: `src/services/__tests__/server-events.test.ts`

- [ ] **Step 1: Write the failing test**

Add/extend tests to prove Android sync emits a consistent running/stopped snapshot and does not rely on duplicate callers.

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
npm test -- server-events
```

Expected: FAIL where duplicate or inconsistent Android sync behavior remains.

- [ ] **Step 3: Write minimal implementation**

Make Android sync payload shaping explicit and avoid conflicting multiple sources of service-state emission while preserving current visible behavior.

- [ ] **Step 4: Run test to verify it passes**

Run the same command and expect PASS.

### Task 5: Full verification and direction-2 handoff readiness

**Files:**
- Modify: `docs/superpowers/specs/2026-03-27-android-service-residency-hardening-design.md`
- Modify: `docs/superpowers/plans/2026-03-27-android-service-residency-hardening.md`

- [ ] **Step 1: Run focused Android tests**

Run:

```bash
./gradlew :app:testUniversalDebugUnitTest --tests "com.gjk.cameraftpcompanion.AndroidServiceStateCoordinatorTest" --tests "com.gjk.cameraftpcompanion.bridges.ServerStateBridgeTest"
```

Expected: PASS.

- [ ] **Step 2: Run frontend targeted tests**

Run:

```bash
npm test -- server-events
```

Expected: PASS.

- [ ] **Step 3: Run required project verification**

Run:

```bash
./build.sh windows android
```

Expected: both builds succeed.

- [ ] **Step 4: Re-check adb background residency behavior**

Verify that when the FTP server is running, the foreground notification remains present after the app is backgrounded and no new lifecycle crash signatures appear in logcat.
