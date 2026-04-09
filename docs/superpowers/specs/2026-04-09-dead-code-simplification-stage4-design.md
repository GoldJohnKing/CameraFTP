# Dead Code & Simplification Cleanup — Stage 4

## Scope

Remove confirmed dead code and simplify redundant patterns across TypeScript, Rust, and Kotlin layers. Approach: 3 batches by language, each independently verified.

## Batch 1: TypeScript

### 1.1 Remove `validatePort()` indirection
- **File**: `src/utils/validation.ts` — delete entire file
- `validatePort()` is only called by `parsePortInput()` in `usePortCheck.ts`, which already performs its own range check (`minPort`/`maxPort`). The `1-65535` range in `validatePort` is redundant.
- Inline the port validation logic directly into `parsePortInput` if needed, or rely on existing range checks.

### 1.2 Inline `AutoStartToggle` component
- **File**: `src/components/AutoStartToggle.tsx` — delete entire file
- Replace usage in `ConfigCard.tsx` with direct `<ToggleSwitch>` usage (hardcode `label="开机自启动"` and `description`).

### 1.3 Extract shared `preview-config-changed` event listener
- **Files**: `PreviewWindow.tsx:94-106`, `PreviewConfigCard.tsx:28-38`
- Create `usePreviewConfigListener(callback)` hook in `src/hooks/` to eliminate duplicate Tauri event listener setup/teardown.

## Batch 2: Rust

### 2.1 Dead code removal

| Item | File | Action |
|------|------|--------|
| `InsertResult` struct | `ftp/android_mediastore/types.rs:42` | Delete struct and re-export from `mod.rs` |
| `FtpAuthConfig::is_anonymous()` | `ftp/types.rs:65` | Delete method |
| `FtpServerHandle::get_snapshot()` | `ftp/server.rs:131` | Delete method |
| Unused `tracing::error` import | `commands/server.rs:6` | Remove import |

### 2.2 Merge duplicate `#[cfg(test)]` method pairs in `backend.rs`
- `normalize_path`, `resolve_path`, `validate_path` each have identical `#[cfg(test)]` (pub) and `#[cfg(not(test))]` (private) implementations.
- Merge into single `pub(crate)` implementations.

### 2.3 Extract duplicated auth credential extraction
- `commands/server.rs:69-80` and `ftp/server.rs:635-647` contain near-identical `(username, password_info)` extraction logic.
- Add `to_display_credentials()` method to `FtpAuthConfig`, unify both call sites.

### 2.4 Inline thin free functions in `platform/android.rs`
- `get_storage_info()`, `check_permission_status()`, `ensure_storage_ready()` are public free functions only called from the `impl PlatformService` block in the same file.
- Inline into the trait impl methods.

## Batch 3: Kotlin

### 3.1 Dead code removal

| Item | File | Lines | Action |
|------|------|-------|--------|
| `deleteDiskEntries()` | `ThumbnailCacheV2.kt` | 182-191 | Delete method |
| `shouldRequestDeleteConfirmation(apiLevel, throwable)` | `GalleryBridge.kt` | 44-50 | Delete unused overload |
| `openViewer()` delegate | `ImageViewerBridge.kt` | 23-25 | Delete; TS side will use `openOrNavigateTo` directly |
| `totalRequests`/`cacheHits` write-only counters | `ThumbnailPipelineManager.kt` | 140-141 | Delete fields and related increment logic |
| Unused `import android.app.Activity` | `GalleryBridgeV2.kt` | 9 | Remove import |
| Unused `import android.widget.Toast` (use short form) | `PermissionBridge.kt` | 20, 271, 308, 322, 336 | Remove FQN, use imported `Toast` |

### 3.2 Toast simplification in `PermissionBridge.kt`
- Replace `android.widget.Toast.makeText()` with `Toast.makeText()` at lines 271, 308, 322, 336.

### 3.3 Sync TS `image-open.ts` after `openViewer` removal
- Remove `openViewer` call path, use `openOrNavigateTo` directly.

## Out of Scope

- `MediaStoreBridge` class → `object` conversion (architectural)
- `BaseJsBridge` generic parameter change to `MainActivity` (affects all subclasses)
- `StatsActor` no-op commands simplification (needs deep event flow understanding)
- `gallery-v2.ts` type exports demotion to module-private (documentation value)
- Test duplication between `tests.rs` and module-level tests (low priority)

## Verification

Each batch verified independently:
- **Batch 1**: `bun test` + frontend build
- **Batch 2**: `./build.sh windows android`
- **Batch 3**: Android build via `./build.sh android`
