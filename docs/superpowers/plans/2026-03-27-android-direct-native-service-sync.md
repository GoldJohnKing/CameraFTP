# Android Direct Native Service Sync Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove Android foreground-service control's dependency on WebView by making Rust directly fan out service state to Android native code, while keeping cross-platform Tauri UI events unchanged.

**Architecture:** Keep Rust as the only source of server lifecycle and stats state. Preserve the existing Tauri event path for UI on all platforms, but add an Android-only platform callback that forwards running/stopped/stats snapshots to `AndroidServiceStateCoordinator` and `FtpForegroundService`. Migrate gradually: add the direct native path, verify it, then remove frontend ownership of Android service control.

**Tech Stack:** Rust, Tauri v2, Kotlin Android foreground service, TypeScript frontend tests

---

### Task 1: Add Rust platform hook for Android-native service sync

**Files:**
- Modify: `src-tauri/src/platform/traits.rs`
- Modify: `src-tauri/src/platform/windows.rs`
- Modify: `src-tauri/src/platform/android.rs`
- Test: `src-tauri/src/platform/android.rs`

- [ ] **Step 1: Write the failing test**

Add or extend a Rust/platform test to express the new contract:

```rust
#[test]
fn android_platform_exposes_native_service_sync_hook() {
    let source = include_str!("traits.rs");
    assert!(source.contains("sync_android_service_state"));
}
```

Also add/update Android platform guard coverage so the old WebView-driven service update event is still absent.

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
./build.sh windows android
```

Expected: build/test phase fails until the new trait method is defined and implemented.

- [ ] **Step 3: Write minimal implementation**

Add a new platform hook with a no-op Windows implementation and Android implementation stub that will later call into native Android coordinator code.

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
./build.sh windows android
```

Expected: build succeeds with the new platform hook in place.

### Task 2: Fan out Rust server lifecycle/state directly to Android native path

**Files:**
- Modify: `src-tauri/src/ftp/events.rs`
- Modify: `src-tauri/src/ftp/server_factory.rs`
- Modify: `src-tauri/src/platform/android.rs`

- [ ] **Step 1: Write the failing test**

Add Rust-side tests that prove server lifecycle/state transitions continue emitting UI events while also invoking the Android native platform sync path from the same event source.

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
./build.sh windows android
```

Expected: failing tests show the Android native sync is not yet called from Rust event handling.

- [ ] **Step 3: Write minimal implementation**

Hook `ServerStarted`, `ServerStopped`, and `StatsUpdated` into the new platform callback while preserving the existing Tauri UI events.

- [ ] **Step 4: Run test to verify it passes**

Run the same verification command and expect success.

### Task 3: Wire Android platform callback into Kotlin coordinator/service

**Files:**
- Modify: `src-tauri/src/platform/android.rs`
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/AndroidServiceStateCoordinator.kt`
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/FtpForegroundService.kt`
- Test: `src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/AndroidServiceStateCoordinatorTest.kt`
- Create: `src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/FtpForegroundServiceTest.kt`

- [ ] **Step 1: Write the failing test**

Add Android tests that prove native service state can be started, updated, and stopped from the direct native path without WebView participation.

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
./gradlew :app:testUniversalDebugUnitTest --tests "com.gjk.cameraftpcompanion.AndroidServiceStateCoordinatorTest" --tests "com.gjk.cameraftpcompanion.FtpForegroundServiceTest"
```

Expected: FAIL until the Rust→Android direct path is wired into coordinator/service entry points.

- [ ] **Step 3: Write minimal implementation**

Expose a direct native-update API into `AndroidServiceStateCoordinator`, and keep `FtpForegroundService` consuming coordinator state only.

- [ ] **Step 4: Run test to verify it passes**

Run the same command and expect PASS.

### Task 4: Remove frontend ownership of Android service control

**Files:**
- Modify: `src/services/android-server-state-sync.ts`
- Modify: `src/services/server-events.ts`
- Modify: `src/stores/serverStore.ts`
- Modify: `src/types/global.ts`
- Test: `src/services/__tests__/server-events.test.ts`
- Test: `src/stores/__tests__/serverStore.characterization.test.ts`
- Test: `src/services/__tests__/android-server-state-sync.test.ts`

- [ ] **Step 1: Write the failing test**

Update frontend tests so they fail if frontend code still acts as the Android service control plane instead of UI-only state handling.

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
npm test -- android-server-state-sync server-events serverStore.characterization
```

Expected: FAIL until frontend Android service-control responsibilities are removed or downgraded to compatibility-only behavior.

- [ ] **Step 3: Write minimal implementation**

Keep frontend updates for UI/store behavior, but remove Android service-control ownership from store/event sync. Downgrade or remove `ServerStateAndroid` bridge usage from frontend code.

- [ ] **Step 4: Run test to verify it passes**

Run the same command and expect PASS.

### Task 5: Remove obsolete WebView-driven Android service path

**Files:**
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/MainActivity.kt`
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/ServerStateBridge.kt`
- Modify: `src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/bridges/ServerStateBridgeTest.kt`

- [ ] **Step 1: Write the failing test**

Add or adjust Android tests to fail if `MainActivity` / `ServerStateBridge` still own the Android service-control path.

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
./gradlew :app:testUniversalDebugUnitTest --tests "com.gjk.cameraftpcompanion.bridges.ServerStateBridgeTest"
```

Expected: FAIL until the obsolete WebView-driven service path is removed or reduced to a non-owning compatibility shim.

- [ ] **Step 3: Write minimal implementation**

Remove the obsolete primary-control role from `MainActivity` / `ServerStateBridge`, leaving only compatibility behavior if still needed during transition.

- [ ] **Step 4: Run test to verify it passes**

Run the same command and expect PASS.

### Task 6: Final verification and cross-platform cutover check

**Files:**
- Modify: `docs/superpowers/specs/2026-03-27-android-direct-native-service-sync-design.md`
- Modify: `docs/superpowers/plans/2026-03-27-android-direct-native-service-sync.md`

- [ ] **Step 1: Run targeted frontend tests**

Run:

```bash
npm test -- android-server-state-sync server-events serverStore.characterization
```

Expected: PASS.

- [ ] **Step 2: Run targeted Android unit tests**

Run:

```bash
./gradlew :app:testUniversalDebugUnitTest --tests "com.gjk.cameraftpcompanion.AndroidServiceStateCoordinatorTest" --tests "com.gjk.cameraftpcompanion.FtpForegroundServiceTest" --tests "com.gjk.cameraftpcompanion.bridges.ServerStateBridgeTest"
```

Expected: PASS.

- [ ] **Step 3: Run required project verification**

Run:

```bash
./build.sh windows android
```

Expected: both builds succeed with no Windows regression.

- [ ] **Step 4: Re-check Android runtime behavior via adb**

Verify that Android foreground notification/state stays correct after backgrounding or WebView loss, and that the app no longer relies on WebView bridge availability for service control.
