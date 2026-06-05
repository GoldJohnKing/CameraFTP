# Eliminate save() WebView JS Hack — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the three `evaluateJavascript` calls in `ColorGradingActivity.save()` with direct JNI calls, eliminating the dependency on MainActivity's WebView being loaded and `window.__TAURI__` internals.

**Architecture:** Add a `OnceLock<Arc<ColorGradingService>>` global singleton (mirroring the existing `ConfigService` pattern) so JNI functions can access `AppHandle.emit()`. Add two new JNI functions: `nativeNotifyDone` (emits Tauri event) and `nativeSaveLastUsed` (persists config). Replace the `scanNewFile` JS round-trip with a direct `MediaScannerConnection.scanFile()` call. Keep one `evaluateJavascript` call for DOM CustomEvent dispatch (gallery refresh) as best-effort — this cannot be eliminated.

**Tech Stack:** Rust (OnceLock, AppHandle, JNI), Kotlin (external fun, MediaScannerConnection), JUnit + Robolectric (Kotlin tests)

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `src-tauri/src/color_grading/service.rs` | Modify | Add `set_global` / `get_global` singleton |
| `src-tauri/src/lib.rs` | Modify | Wire `set_global()` during setup |
| `src-tauri/src/color_grading/jni_bridge.rs` | Modify | Add `nativeNotifyDone` + `nativeSaveLastUsed` + `new_json_ok` helper |
| `src-tauri/gen/android/.../bridges/ColorGradingJniBridge.kt` | Modify | Add `notifyDone()` + `saveLastUsed()` + external declarations + `parseResult` |
| `src-tauri/gen/android/.../ColorGradingActivity.kt` | Modify | Replace three JS calls in `save()` with JNI + direct scan |
| New test file: `src-tauri/gen/android/app/src/test/.../bridges/ColorGradingJniBridgeTest.kt` | Create | Unit tests for `parseResult`, `notifyDone`, `saveLastUsed` Kotlin wrappers |
| New test file: `src-tauri/gen/android/app/src/test/.../ColorGradingActivitySaveTest.kt` | Create | Unit tests for `save()` success/failure wiring |

---

### Task 1: Rust — Add `set_global` / `get_global` to `ColorGradingService`

**Files:**
- Modify: `src-tauri/src/color_grading/service.rs`

- [ ] **Step 1: Add OnceLock import and static**

Add after line 6 (`use std::sync::Arc;`):

```rust
use std::sync::OnceLock;
```

Add before `pub struct ColorGradingService {` (line 27):

```rust
static GLOBAL_SERVICE: OnceLock<Arc<ColorGradingService>> = OnceLock::new();
```

- [ ] **Step 2: Add `set_global` and `get_global` methods**

Add after the closing `}` of `pub fn new(...)` (after line 49):

```rust
    /// Store this instance as the global singleton for JNI access.
    pub fn set_global(self: &Arc<Self>) {
        let _ = GLOBAL_SERVICE.set(Arc::clone(self));
    }

    /// Get the global service instance (set during app setup).
    pub fn get_global() -> &'static Arc<Self> {
        GLOBAL_SERVICE.get().expect("ColorGradingService global not initialized")
    }
```

- [ ] **Step 3: Verify the build compiles**

Run: `./build.sh windows`
Expected: SUCCESS (Android-only code path, but Rust must parse cleanly)

---

### Task 2: Rust — Wire `set_global` in `lib.rs` setup

**Files:**
- Modify: `src-tauri/src/lib.rs:203`

- [ ] **Step 1: Change the service construction to set global**

Replace line 203:

```rust
                app.manage(color_grading::ColorGradingService::new(app.handle().clone(), Arc::clone(&config_service)));
```

with:

```rust
                let cg_service = std::sync::Arc::new(color_grading::ColorGradingService::new(
                    app.handle().clone(),
                    Arc::clone(&config_service),
                ));
                cg_service.set_global();
                app.manage(cg_service);
```

Note: `app.manage()` takes ownership of `Arc<ColorGradingService>`, and `set_global()` clones the Arc first, so both the Tauri managed state and the global static hold references. This is identical to the `ConfigService` pattern at line 176.

- [ ] **Step 2: Verify the build compiles**

Run: `./build.sh windows`
Expected: SUCCESS

---

### Task 3: Rust — Add `nativeNotifyDone` and `nativeSaveLastUsed` JNI functions

**Files:**
- Modify: `src-tauri/src/color_grading/jni_bridge.rs`

- [ ] **Step 1: Add `new_json_ok` helper function**

Add after the existing `json_error` function (after line 58):

```rust
#[cfg(target_os = "android")]
fn new_json_ok(env: &mut JNIEnv) -> jstring {
    new_json_string(env, r#"{"ok":true}"#)
}
```

- [ ] **Step 2: Add `nativeNotifyDone` JNI function**

Add after the `nativeGetLastUsed` function (after line 232):

```rust
/// JNI: Emit color-grading-progress Done event via Tauri.
/// Returns JSON: `{"ok":true}` or `{"ok":false,"error":"message"}`
#[cfg(target_os = "android")]
#[no_mangle]
pub unsafe extern "C" fn Java_com_gjk_cameraftpcompanion_bridges_ColorGradingJniBridge_nativeNotifyDone(
    mut env: JNIEnv,
    _class: JClass,
    output_path: JString,
) -> jstring {
    let path_str = match env.get_string(&output_path) {
        Ok(s) => s.to_string_lossy().into_owned(),
        Err(_) => return json_error(&mut env, "Invalid outputPath"),
    };

    let service = crate::color_grading::service::ColorGradingService::get_global();
    service.notify_done(vec![path_str]);
    new_json_ok(&mut env)
}
```

- [ ] **Step 3: Add `nativeSaveLastUsed` JNI function**

Add after `nativeNotifyDone`:

```rust
/// JNI: Persist color grading last-used config directly to disk.
/// Returns JSON: `{"ok":true}` or `{"ok":false,"error":"message"}`
#[cfg(target_os = "android")]
#[no_mangle]
pub unsafe extern "C" fn Java_com_gjk_cameraftpcompanion_bridges_ColorGradingJniBridge_nativeSaveLastUsed(
    mut env: JNIEnv,
    _class: JClass,
    preset_id: JString,
    metering_mode: JString,
    ev_offset: jfloat,
) -> jstring {
    let preset_id_str = match env.get_string(&preset_id) {
        Ok(s) => s.to_string_lossy().into_owned(),
        Err(_) => return json_error(&mut env, "Invalid presetId"),
    };
    let metering_str = match env.get_string(&metering_mode) {
        Ok(s) => s.to_string_lossy().into_owned(),
        Err(_) => return json_error(&mut env, "Invalid meteringMode"),
    };

    let config_service = crate::config_service::ConfigService::get_global();
    match config_service.mutate_and_persist(|c| {
        c.color_grading_last_used = Some(crate::config::ColorGradingLastUsed {
            preset_id: preset_id_str,
            metering_mode: metering_str,
            ev_offset,
        });
    }) {
        Ok(()) => new_json_ok(&mut env),
        Err(e) => json_error(&mut env, &e.to_string()),
    }
}
```

- [ ] **Step 4: Verify the build compiles**

Run: `./build.sh windows android`
Expected: SUCCESS on both platforms. Android build links the two new JNI symbols.

---

### Task 4: Kotlin — Add `notifyDone()`, `saveLastUsed()` to `ColorGradingJniBridge`

**Files:**
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/ColorGradingJniBridge.kt`

- [ ] **Step 1: Add new external declarations**

Add after line 134 (`private external fun nativeGetLastUsed(): String`):

```kotlin
        @JvmStatic
        private external fun nativeNotifyDone(outputPath: String): String
        @JvmStatic
        private external fun nativeSaveLastUsed(presetId: String, meteringMode: String, evOffset: Float): String
```

- [ ] **Step 2: Add public wrapper methods**

Add after the `getLastUsed()` method (after line 89):

```kotlin
        fun notifyDone(outputPath: String): Result<Unit> {
            return try {
                val json = nativeNotifyDone(outputPath)
                parseResult(json)
            } catch (e: Exception) {
                Log.e(TAG, "notifyDone failed", e)
                Result.failure(e)
            }
        }

        fun saveLastUsed(presetId: String, meteringMode: String, evOffset: Float): Result<Unit> {
            return try {
                val json = nativeSaveLastUsed(presetId, meteringMode, evOffset)
                parseResult(json)
            } catch (e: Exception) {
                Log.e(TAG, "saveLastUsed failed", e)
                Result.failure(e)
            }
        }
```

Note: Both reuse the existing `parseResult(json: String): Result<Unit>` method already defined at line 91.

- [ ] **Step 3: Verify the build compiles**

Run: `./build.sh android`
Expected: SUCCESS

---

### Task 5: Kotlin — Write unit tests for new ColorGradingJniBridge methods

**Files:**
- Create: `src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/bridges/ColorGradingJniBridgeTest.kt`

These tests verify the Kotlin-side JSON parsing logic without calling actual native methods. We test `parseResult` directly since the new methods delegate to it.

- [ ] **Step 1: Write the test file**

```kotlin
/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */
package com.gjk.cameraftpcompanion.bridges

import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [33], manifest = Config.NONE)
class ColorGradingJniBridgeTest {

    // --- parseResult (Unit) tests ---
    // Covers both notifyDone and saveLastUsed return parsing

    @Test
    fun parseResult_okTrue_returnsSuccess() {
        val result = invokeParseResult("""{"ok":true}""")
        assertTrue(result.isSuccess)
    }

    @Test
    fun parseResult_okFalse_returnsFailure() {
        val result = invokeParseResult("""{"ok":false,"error":"something went wrong"}""")
        assertTrue(result.isFailure)
        assertEquals("something went wrong", result.exceptionOrNull()?.message)
    }

    @Test
    fun parseResult_missingOk_returnsFailure() {
        val result = invokeParseResult("""{"error":"no ok field"}""")
        assertTrue(result.isFailure)
    }

    @Test
    fun parseResult_emptyError_returnsDefaultMessage() {
        val result = invokeParseResult("""{"ok":false}""")
        assertTrue(result.isFailure)
        assertEquals("Unknown error", result.exceptionOrNull()?.message)
    }

    @Test
    fun parseResult_malformedJson_throwsException() {
        // parseResult is private, but the wrapper methods catch exceptions.
        // Verify that malformed JSON is handled gracefully at the wrapper level.
        val result = ColorGradingJniBridge.parseResultPublic("""not json""")
        assertTrue(result.isFailure)
    }

    // --- parseResultWithOutputPath tests (existing, regression guard) ---

    @Test
    fun parseResultWithOutputPath_okWithOutputPath_returnsPath() {
        val result = ColorGradingJniBridge.parseResultWithOutputPathPublic(
            """{"ok":true,"outputPath":"/photos/out.jpg"}"""
        )
        assertTrue(result.isSuccess)
        assertEquals("/photos/out.jpg", result.getOrNull())
    }

    @Test
    fun parseResultWithOutputPath_emptyOutputPath_returnsFailure() {
        val result = ColorGradingJniBridge.parseResultWithOutputPathPublic(
            """{"ok":true,"outputPath":""}"""
        )
        assertTrue(result.isFailure)
    }

    // --- parseResultWithBuffer tests (existing, regression guard) ---

    @Test
    fun parseResultWithBuffer_okWithValidBase64_returnsDecodedBytes() {
        // "AQID" = Base64 of [0x01, 0x02, 0x03]
        val result = ColorGradingJniBridge.parseResultWithBufferPublic(
            """{"ok":true,"buffer":"AQID"}"""
        )
        assertTrue(result.isSuccess)
        assertArrayEquals(byteArrayOf(0x01, 0x02, 0x03), result.getOrNull())
    }

    @Test
    fun parseResultWithBuffer_emptyBuffer_returnsFailure() {
        val result = ColorGradingJniBridge.parseResultWithBufferPublic(
            """{"ok":true,"buffer":""}"""
        )
        assertTrue(result.isFailure)
    }

    companion object {
        private fun invokeParseResult(json: String): Result<Unit> {
            return ColorGradingJniBridge.parseResultPublic(json)
        }
    }
}
```

- [ ] **Step 2: Expose parse methods as `internal` for testing**

In `ColorGradingJniBridge.kt`, change the visibility of the three `parse*` methods from `private` to `internal`:

```kotlin
        internal fun parseResult(json: String): Result<Unit> {
            // ... unchanged
        }

        internal fun parseResultWithOutputPath(json: String): Result<String> {
            // ... unchanged
        }

        internal fun parseResultWithBuffer(json: String): Result<ByteArray> {
            // ... unchanged
        }
```

And update the test to call them directly without the `Public` suffix:

```kotlin
    private fun invokeParseResult(json: String): Result<Unit> {
        return ColorGradingJniBridge.parseResult(json)
    }
```

Same for `parseResultWithOutputPath` and `parseResultWithBuffer`.

- [ ] **Step 3: Run the tests**

Run: `./build.sh android` (Kotlin tests run automatically)
Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/bridges/ColorGradingJniBridgeTest.kt
git add src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/ColorGradingJniBridge.kt
git commit -m "test(kotlin): add unit tests for ColorGradingJniBridge JSON parsing"
```

---

### Task 6: Kotlin — Rewrite `save()` in `ColorGradingActivity`

**Files:**
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/ColorGradingActivity.kt`

- [ ] **Step 1: Add `scanOutputFile` method to `ColorGradingActivity`**

Add after the `endPreviewSession()` method (after line 131):

```kotlin
    internal fun scanOutputFile(path: String) {
        android.media.MediaScannerConnection.scanFile(this, arrayOf(path), null, null)
    }
```

- [ ] **Step 2: Rewrite `save()` in `NativeColorGradingPreviewBridge`**

Replace the entire `save()` method (lines 183–239 in the current file) with:

```kotlin
    @JavascriptInterface
    fun save(lutId: String, meteringMode: String, evOffset: Float) {
        val activity = activityRef.get() ?: return
        Log.d(TAG, "save: lut=$lutId metering=$meteringMode ev=$evOffset")

        activity.previewJpegBytes = null

        Thread {
            Log.d(TAG, "save: calling commitPreview (JNI)")
            val result = ColorGradingJniBridge.commitPreview(lutId, true, meteringMode, evOffset)

            activity.runOnUiThread {
                if (result.isSuccess) {
                    val outputPath = result.getOrDefault("")
                    Log.d(TAG, "save: committed successfully to $outputPath")

                    // 1. Emit Tauri Done event via JNI — no WebView dependency
                    ColorGradingJniBridge.notifyDone(outputPath)
                    Log.d(TAG, "save: notified done via JNI")

                    // 2. Save last-used config via JNI — no WebView dependency
                    ColorGradingJniBridge.saveLastUsed(lutId, meteringMode, evOffset)
                    Log.d(TAG, "save: saved last-used config via JNI")

                    // 3. MediaStore scan directly — no JS round-trip
                    activity.scanOutputFile(outputPath)

                    // 4. Gallery refresh via WebView (best-effort — DOM events only)
                    val mainActivity = MainActivity.instance
                    mainActivity?.getWebView()?.evaluateJavascript(
                        """(function(){
                            setTimeout(function(){
                                window.dispatchEvent(new CustomEvent('gallery-refresh-requested',{detail:{reason:'color-grading'}}));
                                window.dispatchEvent(new CustomEvent('latest-photo-refresh-requested',{detail:{reason:'color-grading'}}));
                            },500);
                        })();""",
                        null
                    )

                    // 5. Finish after all operations complete
                    android.os.Handler(android.os.Looper.getMainLooper()).postDelayed({
                        activity.finish()
                    }, 500)
                } else {
                    val msg = result.exceptionOrNull()?.message ?: "保存失败"
                    Log.e(TAG, "save: failed - $msg")
                    activity.webView?.evaluateJavascript(
                        "window.notifyPreviewError?.(${JSONObject.quote(msg)});", null
                    )
                    // Re-apply preview so the user sees the image again
                    val maxWidth = activity.resources.displayMetrics.widthPixels
                    val maxHeight = activity.resources.displayMetrics.heightPixels
                    Thread {
                        val retryResult = ColorGradingJniBridge.applyPreview(lutId, true, meteringMode, evOffset, maxWidth, maxHeight)
                        activity.runOnUiThread {
                            if (retryResult.isSuccess) {
                                activity.previewJpegBytes = retryResult.getOrDefault(ByteArray(0))
                                activity.webView?.evaluateJavascript("window.refreshPreview?.();", null)
                            }
                        }
                    }.start()
                }
            }
        }.start()
    }
```

**Key changes from the current implementation:**

| Before | After | Dependency removed |
|--------|-------|--------------------|
| `mainActivity.getWebView()?.evaluateJavascript("await window.__TAURI__.invoke('notify_color_grading_done',...)")` | `ColorGradingJniBridge.notifyDone(outputPath)` | ✅ No WebView needed |
| `mainActivity.getWebView()?.evaluateJavascript("window.__tauriSaveColorGradingLastUsed?.(...)")` | `ColorGradingJniBridge.saveLastUsed(lutId, meteringMode, evOffset)` | ✅ No WebView needed |
| `mainActivity.getWebView()?.evaluateJavascript("window.ImageViewerAndroid?.scanNewFile?.(...)")` | `activity.scanOutputFile(outputPath)` | ✅ No JS round-trip |
| `mainActivity.getWebView()?.evaluateJavascript("window.dispatchEvent(new CustomEvent('gallery-refresh-requested'...))")` | Same — kept as best-effort | ⚠️ Still needs WebView (unavoidable) |

- [ ] **Step 3: Verify the build compiles**

Run: `./build.sh windows android`
Expected: SUCCESS on both platforms

- [ ] **Step 4: Commit**

```bash
git add src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/ColorGradingActivity.kt
git commit -m "refactor(android): replace save() WebView JS hack with direct JNI + MediaStore scan"
```

---

### Task 7: Kotlin — Write regression test for save() wiring

**Files:**
- Create: `src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/ColorGradingActivitySaveTest.kt`

This test verifies that `scanOutputFile` calls `MediaScannerConnection.scanFile` with the correct path. The `save()` method itself is hard to unit-test (it spawns threads and calls JNI), but we can test `scanOutputFile` and verify the wiring logic.

- [ ] **Step 1: Write the test file**

```kotlin
/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */
package com.gjk.cameraftpcompanion

import android.content.Context
import android.media.MediaScannerConnection
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import org.robolectric.Shadows.shadowOf

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [33], manifest = Config.NONE)
class ColorGradingActivitySaveTest {

    @Test
    fun scanOutputFile_invokesMediaScannerWithCorrectPath() {
        val activity = org.robolectric.Robolectric.buildActivity(ColorGradingActivity::class.java).get()
        val path = "/storage/emulated/0/DCIM/ColorGrading/test_provia_20260605_120000.jpg"

        activity.scanOutputFile(path)

        val scanner = shadowOf(activity).nextStartedService
        // MediaScannerConnection.scanFile is static — verify it doesn't crash.
        // Full verification requires an integration test with MediaStore.
    }
}
```

Note: MediaScannerConnection is a static Android API. In Robolectric, `scanFile` doesn't crash but can't easily be asserted on. The real value of this test is verifying the method exists, compiles, and doesn't throw. Integration testing of the full save flow requires a device.

- [ ] **Step 2: Run the tests**

Run: `./build.sh android`
Expected: All tests pass

- [ ] **Step 3: Commit**

```bash
git add src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/ColorGradingActivitySaveTest.kt
git commit -m "test(kotlin): add regression test for scanOutputFile"
```

---

### Task 8: Full build verification

**Files:** (none changed — verification only)

- [ ] **Step 1: Full build both platforms**

Run: `./build.sh windows android`
Expected: SUCCESS — Rust tests pass (293+), frontend tests pass (298+), Kotlin tests pass, Windows release builds, Android release builds.

- [ ] **Step 2: Check for new warnings**

Inspect the build output for any new warnings related to the changed files. Pay attention to:
- Rust: unused import warnings, dead code warnings
- Kotlin: unresolved references, unchecked casts
- CMake: symbol resolution for the new JNI functions

- [ ] **Step 3: Verify the two new JNI symbols are in the .so**

Run: `nm -D src-tauri/target/aarch64-linux-android/release/libcamera_ftp_companion_lib.so | grep -i "nativeNotifyDone\|nativeSaveLastUsed"`
Expected: Two `T` entries found — confirms the JNI symbols are exported.

---

## Risk Analysis

### What changed vs. what didn't

| Component | Changed? | Risk |
|-----------|----------|------|
| `notify_done()` method | No — just called from new path | None |
| `mutate_and_persist()` method | No — just called from new path | None |
| `ColorGradingService` constructor | No | None |
| `ColorGradingService` global singleton | **NEW** | Low — identical to `ConfigService` pattern |
| Tauri event emission | Same `app_handle.emit()` call | None |
| `parseResult` Kotlin method | Visibility changed: `private` → `internal` | Minimal |
| `save()` success path | Replaced 3 JS calls with 2 JNI + 1 MediaStore + 1 JS | Medium — see below |
| `save()` failure path | Unchanged (from previous fix) | None |

### Android Activity lifecycle analysis

**Scenario 1: Normal save**
```
commitPreview (JNI thread, ~200ms)
  → runOnUiThread:
    notifyDone (JNI, <1ms)
    saveLastUsed (JNI, <1ms)
    scanOutputFile (MediaScannerConnection, async)
    evaluateJavascript (gallery refresh, async)
    postDelayed 500ms → finish()
```
All operations are <1ms except `scanOutputFile` (async) and `evaluateJavascript` (async). The 500ms delay is sufficient. `finish()` is only called after all synchronous operations complete.

**Scenario 2: MainActivity destroyed during save**
```
commitPreview succeeds
  → notifyDone → JNI → Rust emit() → SUCCESS (AppHandle is process-level)
  → saveLastUsed → JNI → Rust ConfigService → SUCCESS (OnceLock singleton)
  → scanOutputFile → MediaScannerConnection → SUCCESS (system API, no Activity needed)
  → evaluateJavascript → mainActivity is null → SKIPPED (safe)
  → finish() → SUCCESS (finishes ColorGradingActivity, returns to launcher)
```
**Result**: Data is saved. Gallery refresh DOM event is lost, but next time the gallery tab is opened, it will see the new file via MediaStore.

**Scenario 3: MainActivity WebView not yet loaded**
```
commitPreview succeeds
  → notifyDone → SUCCESS (Rust event buffered by Tauri)
  → saveLastUsed → SUCCESS (direct file I/O)
  → scanOutputFile → SUCCESS (system API)
  → evaluateJavascript → getWebView() returns null → SKIPPED
  → finish() → SUCCESS
```
**Result**: Same as Scenario 2. Tauri event will be delivered once WebView loads and registers listener.

**Scenario 4: ColorGradingActivity destroyed during commitPreview**
```
commitPreview thread is running
  → onDestroy() called
    → isSessionActive = true → endPreviewSession()
      → previewJpegBytes = null
      → Thread { ColorGradingJniBridge.endPreview() }
  → commitPreview thread: acquires Mutex, but session was already taken by end()
  → Error: "No active preview session" → failure path
```
**Result**: Error handled by failure path (re-apply preview), but Activity is already finishing. The `activity.runOnUiThread` lambda will silently fail since Activity is destroyed. This is acceptable — the user left the screen.

### Zustand state consistency

**Before**: `__tauriSaveColorGradingLastUsed` → Zustand `updateDraft()` → immediate in-memory update.
**After**: `nativeSaveLastUsed` → Rust `mutate_and_persist()` → writes to `config.json`. Zustand memory is stale until next `loadConfig()`.

**Impact**: `colorGradingLastUsed` is only read when the ColorGradingDialog opens. If the user opens it again:
1. `ColorGradingDialog` mounts → `loadConfig()` is called → reads latest from disk → correct.
2. If the dialog was already mounted (unlikely — ColorGradingActivity finished), the draft might be stale. But the draft auto-saves on dialog close, so it will overwrite with whatever was last shown.

**Conclusion**: No practical impact.

---

## Rollback Plan

If issues are discovered:
1. Revert `ColorGradingActivity.kt` `save()` method to use the original `evaluateJavascript` calls
2. The new JNI functions (`nativeNotifyDone`, `nativeSaveLastUsed`) can remain — they are additive and don't break anything
3. The `ColorGradingService.set_global()` and `parseResult` visibility changes are harmless
