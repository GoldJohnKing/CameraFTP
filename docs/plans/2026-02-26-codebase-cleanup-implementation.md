# Codebase Cleanup Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Remove dead code, eliminate duplication, split files by SRP, and improve performance across frontend, Rust, and Android codebases.

**Architecture:** Clean up existing code without architectural changes. Split large files by responsibility, remove redundant wrapper modules, consolidate duplicate permission handling.

**Tech Stack:** TypeScript/React (Zustand), Rust (Tauri v2), Kotlin (Android)

---

## Commit 1: Frontend Cleanup

### Task 1: Audit and Remove Unused Types

**Files:**
- Read: `src/types/global.ts`, `src/types/index.ts`
- Grep: Find usages of each exported type

**Step 1: Check usage of `types/global.ts` exports**

Search for usages of each exported interface:
- `FileUploadListenerBridge`
- `FileUploadBridge`
- `ServerStateBridge`
- `StorageSettingsBridge`
- `PermissionBridge`
- `PermissionCheckResult`

Run: `grep -r "PermissionCheckResult" src/`
Expected: Find usages, identify if inline definition in permissionStore duplicates this

**Step 2: Check usage of `types/index.ts` exports**

Search for:
- `StorageInfo`
- `PermissionStatus`
- `ServerStartCheckResult`

**Step 3: Remove unused type exports**

If any types are unused, remove them from the files.

**Step 4: Verify frontend builds**

Run: `./build.sh frontend`
Expected: Build succeeds

---

### Task 2: Merge useStoragePermission into permissionStore

**Files:**
- Read: `src/hooks/useStoragePermission.ts`
- Modify: `src/stores/permissionStore.ts`
- Delete: `src/hooks/useStoragePermission.ts`

**Step 1: Read both files to understand the duplication**

Run: Read `src/hooks/useStoragePermission.ts` and `src/stores/permissionStore.ts`

**Step 2: Identify unique functionality in useStoragePermission**

The hook likely has:
- Permission check logic
- Permission request logic
- State management

Merge this into permissionStore if not already present.

**Step 3: Update permissionStore with any missing logic**

Add any unique functions from the hook to the store.

**Step 4: Find and update all usages of useStoragePermission**

Run: `grep -r "useStoragePermission" src/`
Expected: Find component(s) using the hook

Update imports to use permissionStore instead.

**Step 5: Delete useStoragePermission.ts**

Remove the file after migration complete.

**Step 6: Verify frontend builds**

Run: `./build.sh frontend`
Expected: Build succeeds

---

### Task 3: Remove Duplicate PermissionCheckResult Definition

**Files:**
- Modify: `src/stores/permissionStore.ts`

**Step 1: Find inline PermissionCheckResult in permissionStore**

Run: `grep -n "PermissionCheckResult" src/stores/permissionStore.ts`

**Step 2: Update to import from types**

Change inline definition to import from `types/global.ts`.

**Step 3: Verify frontend builds**

Run: `./build.sh frontend`
Expected: Build succeeds

---

### Task 4: Split ConfigCard by SRP

**Files:**
- Read: `src/components/ConfigCard.tsx`
- Create: `src/components/PathSelector.tsx`
- Create: `src/components/PortSelector.tsx`
- Create: `src/components/AutoStartToggle.tsx`
- Modify: `src/components/ConfigCard.tsx`

**Step 1: Analyze ConfigCard.tsx structure**

Read the file and identify distinct sections:
- Path selection UI
- Port selection UI
- Autostart toggle UI

**Step 2: Extract PathSelector component**

Create `src/components/PathSelector.tsx` with path selection logic and UI.

```typescript
// Extract path-related props, state, and JSX
interface PathSelectorProps {
  // props identified from ConfigCard
}

export function PathSelector({ ... }: PathSelectorProps) {
  // path selection logic
}
```

**Step 3: Extract PortSelector component**

Create `src/components/PortSelector.tsx` with port selection logic and UI.

**Step 4: Extract AutoStartToggle component**

Create `src/components/AutoStartToggle.tsx` with autostart toggle logic and UI.

**Step 5: Update ConfigCard to use new components**

Refactor `ConfigCard.tsx` to compose the new sub-components.

**Step 6: Verify frontend builds**

Run: `./build.sh frontend`
Expected: Build succeeds

---

### Task 5: Cache Platform Value in configStore

**Files:**
- Modify: `src/stores/configStore.ts`

**Step 1: Find loadPlatform function**

Run: `grep -n "loadPlatform" src/stores/configStore.ts`

**Step 2: Add platform caching**

If platform is fetched on every call, cache it after first load:

```typescript
let cachedPlatform: string | null = null;

// In loadPlatform:
if (cachedPlatform) return cachedPlatform;
// ... fetch and cache
```

**Step 3: Verify frontend builds**

Run: `./build.sh frontend`
Expected: Build succeeds

---

### Task 6: Remove Unused Imports (Frontend)

**Files:**
- All `.ts` and `.tsx` files in `src/`

**Step 1: Check for unused imports warnings**

The TypeScript compiler should warn about unused imports.

**Step 2: Remove unused imports**

Clean up any flagged imports.

**Step 3: Verify frontend builds**

Run: `./build.sh frontend`
Expected: Build succeeds, no warnings

---

### Task 7: Commit Frontend Changes

**Step 1: Stage all frontend changes**

```bash
git add src/
```

**Step 2: Create commit**

```bash
git commit -m "refactor(frontend): cleanup dead code and consolidate permissions

- Merge useStoragePermission into permissionStore
- Remove duplicate PermissionCheckResult definition
- Split ConfigCard into PathSelector, PortSelector, AutoStartToggle
- Cache platform value in configStore
- Remove unused imports and types"
```

---

## Commit 2: Rust Backend Cleanup

### Task 8: Analyze storage_permission.rs

**Files:**
- Read: `src-tauri/src/storage_permission.rs`
- Read: `src-tauri/src/commands.rs`
- Read: `src-tauri/src/lib.rs`

**Step 1: Verify storage_permission.rs is redundant**

Read the file and confirm all functions just call `platform::get_platform().*`.

**Step 2: Find all commands using storage_permission**

Run: `grep -n "storage_permission::" src-tauri/src/commands.rs`

**Step 3: Find module registration**

Run: `grep -n "storage_permission" src-tauri/src/lib.rs`

---

### Task 9: Update commands.rs to Call Platform Directly

**Files:**
- Modify: `src-tauri/src/commands.rs`

**Step 1: Update imports**

Remove `use crate::storage_permission::*;`
Add `use crate::platform::get_platform;`

**Step 2: Update each command**

For each command that called `storage_permission::*`, update to:
```rust
pub async fn get_storage_info() -> Result<StorageInfo, AppError> {
    Ok(get_platform().get_storage_info())
}
```

**Step 3: Verify Rust compiles**

Run: `./build.sh windows`
Expected: Build succeeds

---

### Task 10: Remove storage_permission Module

**Files:**
- Modify: `src-tauri/src/lib.rs`
- Delete: `src-tauri/src/storage_permission.rs`

**Step 1: Remove module declaration from lib.rs**

Remove `mod storage_permission;`

**Step 2: Delete storage_permission.rs**

```bash
rm src-tauri/src/storage_permission.rs
```

**Step 3: Verify Rust compiles**

Run: `./build.sh windows`
Expected: Build succeeds

---

### Task 11: Simplify StopReason Enum (If Unused)

**Files:**
- Read: `src-tauri/src/ftp/types.rs`
- Modify: `src-tauri/src/ftp/types.rs` (if applicable)

**Step 1: Find StopReason usage**

Run: `grep -rn "StopReason" src-tauri/src/`

**Step 2: Evaluate**

If only one variant exists and no future variants planned:
- Either remove the enum and use a simple bool/unit
- Or document why multiple variants are planned

**Step 3: Apply change if warranted**

Make the simplification or add documentation.

**Step 4: Verify Rust compiles**

Run: `./build.sh windows`
Expected: Build succeeds

---

### Task 12: Remove Unused Imports (Rust)

**Files:**
- All `.rs` files in `src-tauri/src/`

**Step 1: Check for unused imports warnings**

Run: `./build.sh windows 2>&1 | grep -i "unused"`

**Step 2: Remove flagged imports**

Clean up any unused imports identified.

**Step 3: Verify Rust compiles**

Run: `./build.sh windows`
Expected: Build succeeds, no warnings

---

### Task 13: Cache Android Path Resolution

**Files:**
- Modify: `src-tauri/src/config.rs`

**Step 1: Find Android path resolution code**

Run: `grep -n "android" src-tauri/src/config.rs`

Look for code that runs on every config load.

**Step 2: Add caching if not present**

If path resolution happens repeatedly, add caching:

```rust
static ANDROID_PATH: OnceCell<PathBuf> = OnceCell::new();

fn get_android_path() -> &'static PathBuf {
    ANDROID_PATH.get_or_init(|| {
        // resolution logic
    })
}
```

**Step 3: Verify Rust compiles**

Run: `./build.sh windows && ./build.sh android`
Expected: Both builds succeed

---

### Task 14: Commit Rust Changes

**Step 1: Stage all Rust changes**

```bash
git add src-tauri/src/
```

**Step 2: Create commit**

```bash
git commit -m "refactor(rust): remove redundant storage_permission module

- Delete storage_permission.rs, call platform directly from commands
- Simplify StopReason enum (document single variant)
- Cache Android path resolution in config
- Remove unused imports"
```

---

## Commit 3: Android/Kotlin Cleanup

### Task 15: Analyze MainActivity.kt Structure

**Files:**
- Read: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/MainActivity.kt`

**Step 1: Identify all classes in MainActivity.kt**

Find:
- `MainActivity` class
- `BaseJsBridge` class
- `FileUploadListener` class
- `FileUploadBridge` class
- `ServerStateBridge` class
- `StorageSettingsBridge` class

**Step 2: Map class dependencies**

Which classes use which? What needs to move together?

---

### Task 16: Create bridges Directory and BaseJsBridge

**Files:**
- Create: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/`
- Create: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/BaseJsBridge.kt`

**Step 1: Create bridges directory**

```bash
mkdir -p src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges
```

**Step 2: Extract BaseJsBridge**

Create `bridges/BaseJsBridge.kt` with the base class:

```kotlin
package com.gjk.cameraftpcompanion.bridges

import android.webkit.JavascriptInterface
import android.app.Activity

abstract class BaseJsBridge(protected val activity: Activity) {
    protected fun runOnUiThread(action: () -> Unit) {
        activity.runOnUiThread(action)
    }
}
```

---

### Task 17: Extract FileUploadBridge

**Files:**
- Create: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/FileUploadBridge.kt`
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/MainActivity.kt`

**Step 1: Create FileUploadBridge.kt**

Extract the `FileUploadBridge` class from MainActivity.kt:

```kotlin
package com.gjk.cameraftpcompanion.bridges

import android.webkit.JavascriptInterface
import com.gjk.cameraftpcompanion.MediaScannerHelper

class FileUploadBridge(activity: Activity) : BaseJsBridge(activity) {
    
    @JavascriptInterface
    fun onFileUploaded(path: String?) {
        path?.let {
            MediaScannerHelper.scanFile(activity, it)
        }
    }
}
```

Note: Remove `FileUploadListener` wrapper - `FileUploadBridge` now directly calls `MediaScannerHelper`.

**Step 2: Update MainActivity.kt**

Remove `FileUploadListener` and `FileUploadBridge` classes.
Add import for new bridge.

---

### Task 18: Extract ServerStateBridge

**Files:**
- Create: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/ServerStateBridge.kt`
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/MainActivity.kt`

**Step 1: Create ServerStateBridge.kt**

Extract the `ServerStateBridge` class:

```kotlin
package com.gjk.cameraftpcompanion.bridges

import android.webkit.JavascriptInterface
import android.webkit.WebView

class ServerStateBridge(
    activity: Activity,
    private val webView: WebView
) : BaseJsBridge(activity) {
    
    @JavascriptInterface
    fun updateState(isRunning: Boolean, address: String?) {
        runOnUiThread {
            webView.evaluateJavascript(
                "window.__serverStateCallback?.($isRunning, '$address')",
                null
            )
        }
    }
}
```

**Step 2: Update MainActivity.kt**

Remove `ServerStateBridge` class.
Add import for new bridge.

---

### Task 19: Extract StorageSettingsBridge

**Files:**
- Create: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/StorageSettingsBridge.kt`
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/MainActivity.kt`

**Step 1: Create StorageSettingsBridge.kt**

Extract the `StorageSettingsBridge` class:

```kotlin
package com.gjk.cameraftpcompanion.bridges

import android.webkit.JavascriptInterface
import com.gjk.cameraftpcompanion.StorageHelper

class StorageSettingsBridge(activity: Activity) : BaseJsBridge(activity) {
    
    @JavascriptInterface
    fun openStorageSettings() {
        StorageHelper.openStorageSettings(activity)
    }
}
```

**Step 2: Update MainActivity.kt**

Remove `StorageSettingsBridge` class.
Add import for new bridge.

---

### Task 20: Update MainActivity to Use New Bridges

**Files:**
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/MainActivity.kt`

**Step 1: Add imports**

```kotlin
import com.gjk.cameraftpcompanion.bridges.*
```

**Step 2: Update bridge registration**

In `onWebViewCreate()`, update to use new bridge classes:

```kotlin
addJsBridge(webView, FileUploadBridge(this), "fileUploadBridge")
addJsBridge(webView, ServerStateBridge(this, webView), "serverStateBridge")
addJsBridge(webView, StorageSettingsBridge(this), "storageSettingsBridge")
```

**Step 3: Remove old inner classes**

Delete `FileUploadListener`, `FileUploadBridge`, `ServerStateBridge`, `StorageSettingsBridge` from MainActivity.kt.
Delete `BaseJsBridge` from MainActivity.kt.

**Step 4: Verify Android builds**

Run: `./build.sh android`
Expected: Build succeeds

---

### Task 21: Update PermissionBridge to Import BaseJsBridge

**Files:**
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/PermissionBridge.kt`

**Step 1: Update import**

Change from local reference to bridges package:

```kotlin
import com.gjk.cameraftpcompanion.bridges.BaseJsBridge
```

**Step 2: Verify Android builds**

Run: `./build.sh android`
Expected: Build succeeds

---

### Task 22: Remove Unused Imports (Kotlin)

**Files:**
- All `.kt` files

**Step 1: Check for unused imports**

The Kotlin compiler should warn about unused imports.

**Step 2: Remove flagged imports**

Clean up any unused imports.

**Step 3: Verify Android builds**

Run: `./build.sh android`
Expected: Build succeeds, no warnings

---

### Task 23: Commit Android Changes

**Step 1: Stage all Android changes**

```bash
git add src-tauri/gen/android/
```

**Step 2: Create commit**

```bash
git commit -m "refactor(android): extract bridges from MainActivity, remove indirection

- Extract BaseJsBridge to bridges package
- Extract FileUploadBridge, ServerStateBridge, StorageSettingsBridge
- Remove FileUploadListener wrapper (FileUploadBridge calls MediaScannerHelper directly)
- MainActivity now handles only activity lifecycle
- Remove unused imports"
```

---

## Commit 4: Performance Improvements

### Task 24: Audit useEffect Dependencies (Frontend)

**Files:**
- Read: `src/stores/serverStore.ts`
- Read: `src/stores/configStore.ts`
- Read: `src/stores/permissionStore.ts`

**Step 1: Review useEffect hooks**

Look for useEffect with potentially incorrect or overly broad dependencies.

**Step 2: Fix any issues**

Update dependency arrays to prevent unnecessary re-renders.

**Step 3: Verify frontend builds**

Run: `./build.sh frontend`
Expected: Build succeeds

---

### Task 25: Verify WakeLock Re-acquisition (Android)

**Files:**
- Read: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/FtpForegroundService.kt`

**Step 1: Find WakeLock code**

Run: `grep -n "WakeLock\|wakeLock" src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/FtpForegroundService.kt`

**Step 2: Verify re-acquisition logic**

The comment says "will be re-acquired" - verify this is actually implemented.

**Step 3: Fix if needed**

If re-acquisition is missing, add it. If it's a stale comment, update the comment.

**Step 4: Verify Android builds**

Run: `./build.sh android`
Expected: Build succeeds

---

### Task 26: Commit Performance Changes

**Step 1: Stage all performance changes**

```bash
git add src/ src-tauri/
```

**Step 2: Create commit**

```bash
git commit -m "perf: caching and optimization improvements

- Audit useEffect dependencies in frontend stores
- Verify WakeLock re-acquisition in FtpForegroundService
- Fix any stale comments or missing implementations"
```

---

## Final Verification

### Task 27: Full Build Verification

**Step 1: Build all targets**

```bash
./build.sh frontend && ./build.sh windows && ./build.sh android
```

Expected: All builds succeed

**Step 2: Verify no warnings**

Check output for any warnings that should be addressed.

---

### Task 28: Final Summary

**Step 1: Review all commits**

```bash
git log --oneline -4
```

Expected: 4 commits matching the plan structure.

**Step 2: Summary**

Report on:
- Lines of code removed
- Files created/deleted
- Any issues encountered
