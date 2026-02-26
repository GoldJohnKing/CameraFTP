# Comprehensive Code Cleanup Design

Date: 2026-02-26
Status: Approved

## Overview

This design documents a comprehensive cleanup effort across the entire codebase:
- Tauri frontend (TypeScript/React)
- Rust backend (Tauri)
- Android Kotlin code

## Goals

1. Remove dead/unused code
2. Eliminate duplicate/redundant code
3. Improve performance
4. Fix potential panic points

## Implementation Approach

All-at-once with organized commits by category:
1. Commit 1: Remove dead code (all platforms)
2. Commit 2: Fix panic points (Rust)
3. Commit 3: Extract duplicate code (all platforms)
4. Commit 4: Performance improvements
5. Commit 5: Remove debug logs

---

## Section 1: Dead Code Removal

### Rust

| Location | Issue | Action |
|----------|-------|--------|
| `src-tauri/src/ftp/error.rs` | Entire `FtpError` enum unused | Delete file, remove from `mod.rs` |
| `src-tauri/src/error.rs:19-20` | `FtpServerError` variant never constructed | Remove variant |
| `src-tauri/src/error.rs:117-125` | `From<crate::ftp::FtpError>` impl | Remove (depends on FtpError removal) |

### TypeScript

| Location | Issue | Action |
|----------|-------|--------|
| `src/types/index.ts:20-21` | `auto_open`, `auto_open_program` unused | Remove from `AppConfig` |
| `src/types/index.ts:32` | `has_all_files_access` in `StorageInfo` unused | Remove |
| `src/types/index.ts:37` | `has_all_files_access` in `PermissionStatus` unused | Remove |
| `src/utils/events.ts:106-108` | `listenerCount` getter unused | Remove |
| `src/utils/events.ts:113-115` | `isCleanedUp` getter unused | Remove |

### Kotlin

| Location | Issue | Action |
|----------|-------|--------|
| `MainActivity.kt:132` | `currentActivity` property unused | Remove property and related code |
| `FtpForegroundService.kt:216` | `isRunning` parameter unused | Remove parameter |

---

## Section 2: Panic Fixes (Rust)

| Location | Issue | Action |
|----------|-------|--------|
| `config.rs:18,28` | `.unwrap()` on mutex lock | Use `expect()` with context |
| `lib.rs:76` | `.unwrap()` on file creation | Use `?` operator |
| `server.rs:225` | `.expect()` inside closure | Propagate error |

---

## Section 3: Duplicate Code Reduction

### Rust Duplicates

| Location | Issue | Action |
|----------|-------|--------|
| `commands.rs:58-65` + `server.rs:354-361` | Identical `ServerInfo` construction | Extract to helper `fn build_server_info()` |
| `platform/android.rs:53-68` + `72-107` | Write-test logic duplicated | Extract to `is_path_writable(path: &str)` |

### TypeScript Duplicates

| Location | Issue | Action |
|----------|-------|--------|
| `ConfigCard.tsx:309-378` + `PermissionDialog.tsx:52-160` | Permission UI duplicated | Extract to `PermissionList` component |
| `serverStore.ts:207-216` + `permissionStore.ts:31-36` | Permission check logic duplicated | Use `permissionStore` as single source |

### Kotlin Inconsistencies

| Location | Issue | Action |
|----------|-------|--------|
| `MainActivity.kt:60` vs `:22` | Mixed UI thread helpers | Standardize to `runOnUi` |
| `MainActivity.kt:302,312` | Double `getInstance()` call | Cache result |

---

## Section 4: Performance Improvements

### Rust

| Location | Issue | Action |
|----------|-------|--------|
| `server.rs:220,225` | Double clone of `root_path` | Clone once |
| `events.rs:67` | `stats.clone()` after reading | Use reference |
| `events.rs:155-161` | `event_type_name()` returns `String` | Return `&'static str` |
| `traits.rs:52-54` | `to_string()` for errors | Use `&'static str` |

### Kotlin

| Location | Issue | Action |
|----------|-------|--------|
| `FtpForegroundService.kt:33` | Log in hot path | Make conditional |
| `FtpForegroundService.kt:206-208` | `String.format()` temp objects | Use string interpolation |

---

## Section 5: Debug Log Removal

### Kotlin (43 logs)

- `FtpForegroundService.kt`: 24 logs
- `MainActivity.kt`: 12 logs
- `PermissionBridge.kt`: 6 logs
- `MediaScannerHelper.kt`: 1 log

### TypeScript

- `global.ts:134`: 1 console.error

---

## Verification

After each commit:
- Run `./build.sh windows && ./build.sh android`
- Verify functionality on both platforms

## Summary Statistics

| Category | Count |
|----------|-------|
| Dead code items | 11 |
| Panic fixes | 4 |
| Duplicate patterns | 6 |
| Performance issues | 6 |
| Debug logs | 44 |
