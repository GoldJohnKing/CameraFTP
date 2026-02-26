# Codebase Cleanup Design

**Date**: 2026-02-26
**Status**: Approved
**Scope**: Frontend, Rust Backend, Android/Kotlin

## Overview

Comprehensive cleanup of the camera-ftp-companion codebase to remove dead code, eliminate duplication, split files by SRP, and improve performance. No architectural changes.

## Commits Structure

```
1. refactor(frontend): cleanup dead code and consolidate permissions
2. refactor(rust): remove redundant storage_permission module
3. refactor(android): extract bridges from MainActivity, remove indirection
4. perf: caching and optimization improvements
```

---

## Commit 1: Frontend Cleanup

### Dead Code Removal
| File | Action |
|------|--------|
| `types/global.ts` | Audit all exported interfaces, remove unused |
| `types/index.ts` | Verify `StorageInfo`, `PermissionStatus`, `ServerStartCheckResult` are used |
| All `.tsx`/`.ts` files | Remove unused imports |

### Duplication Consolidation
| Current State | Action |
|---------------|--------|
| `useStoragePermission.ts` (135 lines) | Merge into `permissionStore.ts` |
| `PermissionCheckResult` defined in both places | Keep in `types/`, remove inline |

### SRP File Split
| File | Split Into |
|------|------------|
| `ConfigCard.tsx` (348 lines) | `ConfigCard.tsx` + `PathSelector.tsx` + `PortSelector.tsx` + `AutoStartToggle.tsx` |

### Performance
- Cache platform value in `configStore.ts` after first load

---

## Commit 2: Rust Backend Cleanup

### Dead Code Removal
| File | Action |
|------|--------|
| `storage_permission.rs` | Delete entirely |
| `commands.rs` | Update to call `platform::get_platform()` directly |
| `lib.rs` | Remove `mod storage_permission` |
| `ftp/types.rs` | Simplify or document `StopReason` (single variant) |
| All `.rs` files | Remove unused imports |

### Performance
- Cache Android path resolution in `config.rs`

---

## Commit 3: Android/Kotlin Cleanup

### Dead Code Removal
| File | Action |
|------|--------|
| `FileUploadListener` class | Remove (unnecessary wrapper) |
| `FileUploadBridge` | Call `MediaScannerHelper` directly |
| All `.kt` files | Remove unused imports |

### SRP File Splits
| From | To |
|------|-----|
| `MainActivity.kt` (303 lines with 4 bridges) | `MainActivity.kt` (activity only) |
| - | `bridges/BaseJsBridge.kt` |
| - | `bridges/FileUploadBridge.kt` |
| - | `bridges/ServerStateBridge.kt` |
| - | `bridges/StorageSettingsBridge.kt` |

---

## Commit 4: Performance Improvements

| Area | Change |
|------|--------|
| Frontend | Audit `useEffect` dependencies |
| Rust | Cache platform value after first detection |
| Android | Verify WakeLock re-acquisition logic |

---

## Impact Summary

| Metric | Before | After |
|--------|--------|-------|
| Frontend files | 24 | 26 |
| Rust files | 19 | 18 |
| Kotlin files | 5 | 10 |
| Dead code removed | - | ~150 lines |
| Duplication removed | - | ~135 lines |

## Constraints

- No architectural changes
- Split files by Single Responsibility Principle
- Keep small files consolidated
- Per-area commits for easier review
