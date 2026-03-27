# Android WebView Teardown Crash Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop the Android app from crashing after long runtimes when the app goes to background or its WebView is destroyed while thumbnail/event callbacks are still active.

**Architecture:** Treat the crash as a lifecycle bug: once the WebView teardown starts, no bridge code may schedule new `evaluateJavascript` work, and long-lived thumbnail workers must be drained/cancelled. Add an explicit Android-side teardown path that invalidates listeners, cancels pending handlers/retries, and shuts down thumbnail dispatch before the WebView becomes unusable.

**Tech Stack:** Kotlin, Android WebView, Robolectric, Tauri Android host

---

### Task 1: Reproduce the lifecycle contract in tests

**Files:**
- Modify: `src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/bridges/GalleryBridgeV2Test.kt`
- Modify: `src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/galleryv2/ThumbnailPipelineManagerTest.kt`

- [ ] **Step 1: Write the failing test**

Add tests that prove teardown stops result delivery and drains pending main-thread flush work.

- [ ] **Step 2: Run test to verify it fails**

Run: `./gradlew testDebugUnitTest --tests "com.gjk.cameraftpcompanion.bridges.GalleryBridgeV2Test" --tests "com.gjk.cameraftpcompanion.galleryv2.ThumbnailPipelineManagerTest"`

Expected: FAIL because teardown APIs / cleanup behavior do not exist yet.

- [ ] **Step 3: Write minimal implementation**

Add explicit teardown methods and state guards in Android bridge/pipeline classes.

- [ ] **Step 4: Run test to verify it passes**

Run: `./gradlew testDebugUnitTest --tests "com.gjk.cameraftpcompanion.bridges.GalleryBridgeV2Test" --tests "com.gjk.cameraftpcompanion.galleryv2.ThumbnailPipelineManagerTest"`

Expected: PASS.

### Task 2: Wire Activity teardown to bridge shutdown

**Files:**
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/MainActivity.kt`
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/GalleryBridgeV2.kt`
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/galleryv2/ThumbnailPipelineManager.kt`

- [ ] **Step 1: Add teardown path before WebView reference is cleared**

Invalidate listeners, cancel queued/running thumbnail work, remove pending handler callbacks, and block future `evaluateJavascript` calls once teardown begins.

- [ ] **Step 2: Ensure retry loops cannot outlive the Activity**

Cancel `postDelayed` retry work for Tauri listener registration during destroy.

- [ ] **Step 3: Run focused Android unit tests**

Run: `./gradlew testDebugUnitTest --tests "com.gjk.cameraftpcompanion.bridges.GalleryBridgeV2Test" --tests "com.gjk.cameraftpcompanion.galleryv2.ThumbnailPipelineManagerTest"`

Expected: PASS.

### Task 3: Full verification

**Files:**
- Modify: `docs/superpowers/plans/2026-03-27-android-webview-teardown-crash.md`

- [ ] **Step 1: Run required project verification**

Run: `./build.sh windows android`

Expected: both builds succeed.

- [ ] **Step 2: Re-check adb exit behavior after install**

Run the app, background it, and confirm logcat no longer shows `FORTIFY: pthread_mutex_lock called on a destroyed mutex` for `com.gjk.cameraftpcompanion`.
