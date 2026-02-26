# Comprehensive Code Cleanup Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Remove dead code, eliminate duplicates, improve performance, and fix panic points across the entire codebase.

**Architecture:** Clean up organized by commit category - dead code first (lowest risk), then panic fixes, duplicates, performance, and finally debug log removal.

**Tech Stack:** Rust (Tauri), TypeScript/React, Kotlin (Android)

---

## Commit 1: Remove Dead Code (All Platforms)

### Task 1.1: Remove Rust FtpError Module

**Files:**
- Delete: `src-tauri/src/ftp/error.rs`
- Modify: `src-tauri/src/ftp/mod.rs:9,18`
- Modify: `src-tauri/src/error.rs:19-20,117-126`

**Step 1: Remove FtpError module from mod.rs**

Edit `src-tauri/src/ftp/mod.rs`:
- Remove line 9: `pub mod error;`
- Remove line 18: `pub use error::FtpError;`

**Step 2: Remove FtpError export from types**

The types export at line 18 should be removed since nothing uses FtpError.

**Step 3: Remove FtpServerError variant and From impl from error.rs**

Edit `src-tauri/src/error.rs`:
- Remove lines 19-20: `FtpServerError(String)` variant
- Remove lines 117-126: `impl From<crate::ftp::FtpError> for AppError`
- Remove `Self::FtpServerError(_)` from `code()` match at line 49
- Remove `Self::FtpServerError(msg)` from `user_message()` match at line 68
- Remove `Self::FtpServerError(_)` from `is_critical()` match at line 82

**Step 4: Delete the FtpError file**

```bash
rm src-tauri/src/ftp/error.rs
```

**Step 5: Verify build**

```bash
./build.sh windows && ./build.sh android
```

**Step 6: Commit**

```bash
git add -A && git commit -m "refactor(rust): remove unused FtpError module and FtpServerError variant"
```

---

### Task 1.2: Remove TypeScript Unused Types

**Files:**
- Modify: `src/types/index.ts:20-21,32,37`

**Step 1: Remove unused AppConfig fields**

Edit `src/types/index.ts`, remove from `AppConfig`:
```typescript
// Remove these lines:
auto_open: boolean;
auto_open_program: string | null;
```

**Step 2: Remove has_all_files_access from StorageInfo**

Edit `src/types/index.ts`, remove from `StorageInfo`:
```typescript
// Remove this line:
has_all_files_access: boolean;
```

**Step 3: Remove has_all_files_access from PermissionStatus**

Edit `src/types/index.ts`, remove from `PermissionStatus`:
```typescript
// Remove this line:
has_all_files_access: boolean;
```

**Step 4: Verify build**

```bash
./build.sh frontend
```

**Step 5: Commit**

```bash
git add src/types/index.ts && git commit -m "refactor(ts): remove unused type fields from AppConfig, StorageInfo, PermissionStatus"
```

---

### Task 1.3: Remove TypeScript Unused EventManager Getters

**Files:**
- Modify: `src/utils/events.ts:103-115`

**Step 1: Remove listenerCount getter**

Remove lines 103-108 from `src/utils/events.ts`:
```typescript
// Remove:
/**
 * 获取当前注册的监听器数量
 */
get listenerCount(): number {
  return unlisteners.length;
},
```

**Step 2: Remove isCleanedUp getter**

Remove lines 110-115 from `src/utils/events.ts`:
```typescript
// Remove:
/**
 * 是否已清理
 */
get isCleanedUp(): boolean {
  return isCleanedUp;
},
```

**Step 3: Verify build**

```bash
./build.sh frontend
```

**Step 4: Commit**

```bash
git add src/utils/events.ts && git commit -m "refactor(ts): remove unused listenerCount and isCleanedUp getters from EventManager"
```

---

### Task 1.4: Remove Kotlin Unused Property

**Files:**
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/MainActivity.kt`

**Step 1: Remove currentActivity companion property**

Find and remove the `currentActivity` property and its usages:
```kotlin
// Remove companion object property:
private var currentActivity: MainActivity? = null

// Remove assignments in onCreate and onDestroy:
currentActivity = this  // in onCreate
currentActivity = null  // in onDestroy
```

**Step 2: Verify build**

```bash
./build.sh android
```

**Step 3: Commit**

```bash
git add src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/MainActivity.kt
git commit -m "refactor(android): remove unused currentActivity property"
```

---

### Task 1.5: Remove Kotlin Unused Parameter

**Files:**
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/FtpForegroundService.kt`

**Step 1: Remove isRunning parameter from updateServerState**

Find `updateServerState(isRunning: Boolean, ...)` and remove the unused parameter.

Update all call sites to not pass `isRunning`.

**Step 2: Verify build**

```bash
./build.sh android
```

**Step 3: Commit**

```bash
git add src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/FtpForegroundService.kt
git commit -m "refactor(android): remove unused isRunning parameter from updateServerState"
```

---

## Commit 2: Fix Panic Points (Rust)

### Task 2.1: Fix Mutex Unwrap in config.rs

**Files:**
- Modify: `src-tauri/src/config.rs:18,28`

**Step 1: Replace unwrap with expect in set_android_config_path**

Change line 18:
```rust
// Before:
let mut config_guard = ANDROID_CONFIG_PATH.lock().unwrap();

// After:
let mut config_guard = ANDROID_CONFIG_PATH.lock()
    .expect("ANDROID_CONFIG_PATH mutex poisoned");
```

**Step 2: Replace unwrap with expect in get_android_config_path**

Change line 28:
```rust
// Before:
ANDROID_CONFIG_PATH.lock().unwrap()

// After:
ANDROID_CONFIG_PATH.lock()
    .expect("ANDROID_CONFIG_PATH mutex poisoned")
```

**Step 3: Verify build**

```bash
./build.sh windows && ./build.sh android
```

**Step 4: Commit**

```bash
git add src-tauri/src/config.rs && git commit -m "fix(rust): use expect instead of unwrap for mutex locks in config.rs"
```

---

### Task 2.2: Fix File Creation Unwrap in lib.rs

**Files:**
- Modify: `src-tauri/src/lib.rs:72-76`

**Step 1: Handle file creation error gracefully**

Replace the unwrap chain with proper fallback:
```rust
// Before:
std::fs::OpenOptions::new()
    .create(true)
    .append(true)
    .open(&log_file_for_writer)
    .unwrap_or_else(|_| std::fs::File::create("/dev/null").unwrap())

// After:
std::fs::OpenOptions::new()
    .create(true)
    .append(true)
    .open(&log_file_for_writer)
    .unwrap_or_else(|_| {
        std::fs::File::create("/dev/null")
            .expect("Failed to create /dev/null")
    })
```

**Step 2: Verify build**

```bash
./build.sh windows && ./build.sh android
```

**Step 3: Commit**

```bash
git add src-tauri/src/lib.rs && git commit -m "fix(rust): handle file creation error gracefully in logging setup"
```

---

### Task 2.3: Fix Expect in FTP Server Closure

**Files:**
- Modify: `src-tauri/src/ftp/server.rs:225`

**Step 1: Propagate error instead of panic**

This is inside a closure passed to `ServerBuilder`. The closure returns `Box<dyn StorageBackend>`. Need to restructure to propagate error:

```rust
// Before (line 224-225):
let result = ServerBuilder::new(Box::new(move || {
    unftp_sbe_fs::Filesystem::new(root_path.clone()).expect("Failed to create filesystem")
}))

// After - create filesystem before closure:
let filesystem = match unftp_sbe_fs::Filesystem::new(root_path.clone()) {
    Ok(fs) => fs,
    Err(e) => {
        error!(error = %e, "Failed to create filesystem");
        {
            let mut status = self.status.write().await;
            *status = ServerStatus::Stopped;
        }
        return Err(AppError::Io(e.to_string()));
    }
};

let result = ServerBuilder::new(Box::new(move || filesystem.clone()))
```

Note: Check if `Filesystem` implements `Clone`. If not, use `Arc<Filesystem>`.

**Step 2: Verify build**

```bash
./build.sh windows && ./build.sh android
```

**Step 3: Commit**

```bash
git add src-tauri/src/ftp/server.rs && git commit -m "fix(rust): propagate filesystem creation error instead of panic"
```

---

## Commit 3: Extract Duplicate Code

### Task 3.1: Extract ServerInfo Helper in Rust

**Files:**
- Modify: `src-tauri/src/commands.rs:58-65`
- Modify: `src-tauri/src/ftp/server.rs:354-361`
- Modify: `src-tauri/src/ftp/types.rs`

**Step 1: Add build_server_info helper to types.rs**

Add to `src-tauri/src/ftp/types.rs`:
```rust
impl ServerInfo {
    pub fn new(ip: String, port: u16) -> Self {
        Self {
            is_running: true,
            ip: ip.clone(),
            port,
            url: format!("ftp://{}:{}", ip, port),
            username: "anonymous".to_string(),
            password_info: "(任意密码)".to_string(),
        }
    }
}
```

**Step 2: Use helper in commands.rs**

Replace the ServerInfo construction with:
```rust
ServerInfo::new(ctx.ip.clone(), ctx.port)
```

**Step 3: Use helper in server.rs**

Replace the ServerInfo construction with:
```rust
ServerInfo::new(ip, addr.port())
```

**Step 4: Verify build**

```bash
./build.sh windows && ./build.sh android
```

**Step 5: Commit**

```bash
git add src-tauri/src/ftp/types.rs src-tauri/src/commands.rs src-tauri/src/ftp/server.rs
git commit -m "refactor(rust): extract ServerInfo::new helper to eliminate duplication"
```

---

### Task 3.2: Extract Path Writable Helper in Android Platform

**Files:**
- Modify: `src-tauri/src/platform/android.rs:53-107`

**Step 1: Create is_path_writable helper**

Add a new helper function:
```rust
fn is_path_writable(path: &Path) -> bool {
    let test_file = path.join(".write_test");
    match std::fs::File::create(&test_file) {
        Ok(_) => {
            let _ = std::fs::remove_file(&test_file);
            true
        }
        Err(_) => false,
    }
}
```

**Step 2: Refactor can_write_to_dcim to use helper**

```rust
fn can_write_to_dcim(&self) -> bool {
    let dcim_path = Path::new("/storage/emulated/0/DCIM");
    dcim_path.exists() && is_path_writable(dcim_path)
}
```

**Step 3: Refactor validate_path_writable to use helper**

Simplify to use the shared helper function.

**Step 4: Verify build**

```bash
./build.sh android
```

**Step 5: Commit**

```bash
git add src-tauri/src/platform/android.rs
git commit -m "refactor(rust): extract is_path_writable helper to eliminate duplication"
```

---

### Task 3.3: Extract PermissionList Component in TypeScript

**Files:**
- Create: `src/components/PermissionList.tsx`
- Modify: `src/components/ConfigCard.tsx:309-378`
- Modify: `src/components/PermissionDialog.tsx:52-160`

**Step 1: Create PermissionList component**

Create `src/components/PermissionList.tsx` with shared permission UI:
```typescript
import { usePermissionStore } from '../stores/permissionStore';

interface PermissionListProps {
  showStorage?: boolean;
  showNotification?: boolean;
  showBattery?: boolean;
}

export function PermissionList({ 
  showStorage = true, 
  showNotification = true, 
  showBattery = true 
}: PermissionListProps) {
  const {
    storage,
    notification,
    batteryOptimization,
    requestStoragePermission,
    requestNotificationPermission,
    requestBatteryOptimization,
  } = usePermissionStore();

  // Render permission items...
}
```

**Step 2: Update ConfigCard to use PermissionList**

Replace the permission section with:
```tsx
<PermissionList />
```

**Step 3: Update PermissionDialog to use PermissionList**

Replace the permission section with:
```tsx
<PermissionList />
```

**Step 4: Verify build**

```bash
./build.sh frontend
```

**Step 5: Commit**

```bash
git add src/components/PermissionList.tsx src/components/ConfigCard.tsx src/components/PermissionDialog.tsx
git commit -m "refactor(ts): extract PermissionList component to eliminate UI duplication"
```

---

### Task 3.4: Fix Kotlin Double getInstance Call

**Files:**
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/MainActivity.kt:302,312`

**Step 1: Cache getInstance result**

```kotlin
// Before:
if (FtpForegroundService.getInstance() == null) {
    // ...
}
FtpForegroundService.getInstance()?.updateServerState(...)

// After:
val service = FtpForegroundService.getInstance()
if (service == null) {
    // ...
    return
}
service.updateServerState(...)
```

**Step 2: Verify build**

```bash
./build.sh android
```

**Step 3: Commit**

```bash
git add src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/MainActivity.kt
git commit -m "refactor(android): cache getInstance result to avoid double call"
```

---

## Commit 4: Performance Improvements

### Task 4.1: Fix Double Clone in server.rs

**Files:**
- Modify: `src-tauri/src/ftp/server.rs:220,225`

**Step 1: Clone once before closure**

```rust
// Before:
let root_path = config.root_path.clone();
// ... later in closure:
unftp_sbe_fs::Filesystem::new(root_path.clone())

// After:
let root_path_for_closure = config.root_path.clone();
// ... in closure:
unftp_sbe_fs::Filesystem::new(root_path_for_closure.clone())
```

Actually, since we already have `root_path` cloned at line 220, we just need to use it directly in the closure instead of cloning again:
```rust
let root_path = config.root_path.clone();
// ... in closure at 225:
unftp_sbe_fs::Filesystem::new(root_path)  // move instead of clone
```

But wait - the closure is `Box::new(move || ...)` so it can capture `root_path` by move. If we need to use it multiple times, we need `Clone`. The issue is line 225 clones again unnecessarily.

Actually, looking more carefully - the closure is `FnMut`, so it might be called multiple times. We need the clone inside. But we're cloning twice before the closure (line 220 and 225).

Let me re-examine... The fix is:
- Line 220: `let root_path = config.root_path.clone();` - needed for closure
- Line 225: `root_path.clone()` - needed because closure is `FnMut` and may be called multiple times

The actual fix is to remove the extra clone at line 220 if not needed elsewhere, or use `Arc<PathBuf>` for shared ownership.

For simplicity, let's just note this as a minor optimization and skip it if too complex.

**Alternative approach: Skip this task as too complex for the benefit.**

Mark as: **SKIPPED** - The double clone is necessary because the closure is `FnMut` and needs the path each invocation. Using `Arc` would add complexity for minimal gain.

---

### Task 4.2: Return &static str from event_type_name

**Files:**
- Modify: `src-tauri/src/ftp/events.rs:155-161`

**Step 1: Change return type to &'static str**

```rust
// Before:
fn event_type_name(&self) -> String {

// After:
fn event_type_name(&self) -> &'static str {
    match self {
        Self::ServerStarted { .. } => "server_started",
        Self::ServerStopped { .. } => "server_stopped",
        Self::ClientConnected { .. } => "client_connected",
        // ... etc
    }
}
```

**Step 2: Verify build**

```bash
./build.sh windows && ./build.sh android
```

**Step 3: Commit**

```bash
git add src-tauri/src/ftp/events.rs
git commit -m "perf(rust): return &'static str from event_type_name to avoid allocation"
```

---

### Task 4.3: Fix String.format in Kotlin

**Files:**
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/FtpForegroundService.kt:206-208`

**Step 1: Replace String.format with string interpolation**

```kotlin
// Before:
String.format("%.1f KB", bytes / 1024.0)

// After:
"${(bytes / 1024.0).let { "%.1f KB".format(it) }}"
// Or simpler if precision isn't critical:
"${bytes / 1024} KB"
```

Actually, `String.format` is fine for formatting. The issue mentioned is creating temporary objects, but string interpolation also creates temporary objects. Let me check what the actual code does...

If it's just formatting a number, we can use:
```kotlin
"%.1f KB".format(bytes / 1024.0)
```

This uses Kotlin's extension function which is cleaner than `String.format()`.

**Step 2: Verify build**

```bash
./build.sh android
```

**Step 3: Commit**

```bash
git add src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/FtpForegroundService.kt
git commit -m "style(android): use Kotlin format extension instead of String.format"
```

---

## Commit 5: Remove Debug Logs

### Task 5.1: Remove Kotlin Debug Logs (FtpForegroundService)

**Files:**
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/FtpForegroundService.kt`

**Step 1: Remove all Log.d statements**

Remove approximately 24 `Log.d(TAG, ...)` calls throughout the file.

Keep only error/warning logs (Log.e, Log.w) if they're critical.

**Step 2: Verify build**

```bash
./build.sh android
```

**Step 3: Commit**

```bash
git add src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/FtpForegroundService.kt
git commit -m "chore(android): remove debug logs from FtpForegroundService"
```

---

### Task 5.2: Remove Kotlin Debug Logs (MainActivity)

**Files:**
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/MainActivity.kt`

**Step 1: Remove all Log.d statements**

Remove approximately 12 `Log.d(TAG, ...)` calls throughout the file.

**Step 2: Verify build**

```bash
./build.sh android
```

**Step 3: Commit**

```bash
git add src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/MainActivity.kt
git commit -m "chore(android): remove debug logs from MainActivity"
```

---

### Task 5.3: Remove Kotlin Debug Logs (PermissionBridge)

**Files:**
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/PermissionBridge.kt`

**Step 1: Remove all Log.d statements**

Remove approximately 6 `Log.d(TAG, ...)` calls throughout the file.

**Step 2: Verify build**

```bash
./build.sh android
```

**Step 3: Commit**

```bash
git add src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/PermissionBridge.kt
git commit -m "chore(android): remove debug logs from PermissionBridge"
```

---

### Task 5.4: Remove Kotlin Debug Logs (MediaScannerHelper)

**Files:**
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/MediaScannerHelper.kt`

**Step 1: Remove Log.d statement**

Remove 1 `Log.d(TAG, ...)` call.

**Step 2: Verify build**

```bash
./build.sh android
```

**Step 3: Commit**

```bash
git add src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/MediaScannerHelper.kt
git commit -m "chore(android): remove debug log from MediaScannerHelper"
```

---

### Task 5.5: Remove TypeScript console.error

**Files:**
- Modify: `src/types/global.ts:134`

**Step 1: Remove or replace console.error**

Either remove the console.error or replace with proper error handling.

**Step 2: Verify build**

```bash
./build.sh frontend
```

**Step 3: Commit**

```bash
git add src/types/global.ts
git commit -m "chore(ts): remove console.error from global.ts"
```

---

## Final Verification

After all commits:

```bash
./build.sh windows && ./build.sh android && ./build.sh frontend
```

---

## Summary

| Commit | Description | Tasks |
|--------|-------------|-------|
| 1 | Remove Dead Code | 5 tasks |
| 2 | Fix Panic Points | 3 tasks |
| 3 | Extract Duplicate Code | 4 tasks |
| 4 | Performance Improvements | 3 tasks (1 skipped) |
| 5 | Remove Debug Logs | 5 tasks |

**Total: 19 tasks**
