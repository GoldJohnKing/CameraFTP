# Code Quality Cleanup Design

**Date**: 2026-03-01
**Status**: Approved
**Scope**: Dead code, duplicates, performance, debounce bug fix

---

## Summary

Comprehensive code quality cleanup across TypeScript frontend, Rust backend, and Android Kotlin code. Removes ~250 LOC of dead/redundant code, consolidates duplicate patterns, and fixes performance issues.

---

## Section 1: Dead Code Removal

### 1.1 Delete Unused Component

**File**: `src/components/PortSelector.tsx`

- **Action**: Delete entire file (128 LOC)
- **Reason**: Component is exported but never imported anywhere
- **Risk**: None - no references found

### 1.2 Remove Unused npm Dependencies

**File**: `package.json`

- Remove `@tauri-apps/plugin-process`
- Remove `@tauri-apps/plugin-shell`
- **Reason**: No imports or usage found in codebase

### 1.3 Remove Dead Rust Method

**File**: `src-tauri/src/ftp/listeners.rs:24`

- Remove `set_app_handle()` method from `FtpDataListener`
- **Reason**: Method defined but never called; app_handle passed via constructor

---

## Section 2: Duplicate Code Consolidation

### 2.1 Extract Shared Port Validation

**New File**: `src/utils/validation.ts`

```typescript
export function validatePort(value: string): number | null {
  const port = parseInt(value, 10);
  if (isNaN(port) || port < 1 || port > 65535) return null;
  return port;
}
```

**Update**:
- `src/components/AdvancedConnectionConfig.tsx:93-109` → use shared function
- `src/components/PortSelector.tsx` → deleted, no update needed

### 2.2 Create usePortCheck Hook

**New File**: `src/hooks/usePortCheck.ts`

Combines validation + availability check pattern used in:
- `AdvancedConnectionConfig.tsx:190-209`
- `PortSelector.tsx:57-78` (being deleted)

```typescript
export function usePortCheck() {
  const [isChecking, setIsChecking] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const checkPort = async (port: number): Promise<boolean> => {
    // Combined logic
  };

  return { checkPort, isChecking, error };
}
```

### 2.3 Consolidate Rust Server Command Pattern

**File**: `src-tauri/src/ftp/server.rs`

Create helper method to eliminate 4 duplicate patterns:

```rust
async fn send_command<T: Send + 'static>(&self, cmd: ServerCommand<T>) -> Result<T, AppError> {
    let (tx, rx) = oneshot::channel();
    if self.tx.send(cmd.with_response(tx)).await.is_err() {
        return Err(AppError::ServerNotRunning);
    }
    rx.await.map_err(|_| AppError::ServerNotRunning)?
}
```

### 2.4 Consolidate Platform Service Commands

**File**: `src-tauri/src/commands.rs`

7 commands are thin wrappers around `get_platform_service()`:
- `get_storage_info`
- `check_permission_status`
- `request_all_files_permission`
- `request_manage_storage_permission`
- `open_storage_settings`
- `get_preview_config`
- `set_preview_config`

**Option A**: Use macro for boilerplate reduction
**Option B**: Keep as-is (explicit is sometimes better)

**Decision**: Keep as-is for explicitness and type safety.

---

## Section 3: Performance Optimizations

### 3.1 Use Arc<PathBuf> in FtpDataListener

**File**: `src-tauri/src/ftp/listeners.rs:15`

Change:
```rust
pub save_path: std::path::PathBuf,
```
To:
```rust
pub save_path: std::sync::Arc<std::path::PathBuf>,
```

This avoids cloning PathBuf on every FTP event.

### 3.2 Return Arc<Vec<FileInfo>> from FileIndexService

**File**: `src-tauri/src/file_index/service.rs:190`

Change:
```rust
pub async fn get_files(&self) -> Vec<FileInfo> {
    let index = self.index.read().await;
    index.files.clone()
}
```
To:
```rust
pub async fn get_files(&self) -> std::sync::Arc<Vec<FileInfo>> {
    let index = self.index.read().await;
    index.files.clone() // Now Arc::clone, O(1)
}
```

Update `FileInfo` wrapper in `types.rs` to use `Arc<Vec<FileInfo>>`.

### 3.3 Add memo() to PreviewWindowContent

**File**: `src/components/PreviewWindow.tsx`

- Wrap `PreviewWindowContent` in `memo()`
- Extract `loadExifInfo` with `useCallback`
- Memoize filename extraction with `useMemo`

### 3.4 Move PERMISSION_CONFIGS Outside Component

**File**: `src/components/PermissionList.tsx:113`

Move the `PERMISSION_CONFIGS` array outside the component function to prevent recreation on every render.

### 3.5 Memoize Default Config in ConfigCard

**File**: `src/components/ConfigCard.tsx:173-177`

Extract inline object to module-level constant or use `useMemo`.

---

## Section 4: Bug Fixes

### 4.1 Fix Debounce Singleton Closure Bug

**File**: `src/stores/configStore.ts:76-87`

Current issue: `debouncedSave` captures first `saveFn` forever.

Fix: Use factory pattern per-store instance:

```typescript
const createDebouncedSave = (saveFn: (config: AppConfig) => Promise<void>) => {
  return debounce(async (config: AppConfig) => {
    await saveFn(config);
  }, DEBOUNCE_DELAY);
};

// In store:
const debouncedSave = createDebouncedSave(saveConfig);
```

---

## Implementation Order

1. **Dead code removal** (safe, no dependencies)
2. **Bug fix: debounce** (self-contained)
3. **Duplicate consolidation** (requires new files)
4. **Performance optimizations** (requires testing)

---

## Verification

After each section:
- TypeScript: `./build.sh frontend`
- Rust: `./build.sh windows && ./build.sh android`
- Manual: Verify app functionality

---

## Files Changed Summary

| Action | Files |
|--------|-------|
| Delete | `src/components/PortSelector.tsx` |
| Create | `src/utils/validation.ts`, `src/hooks/usePortCheck.ts` |
| Modify | `package.json`, `src-tauri/src/ftp/listeners.rs`, `src-tauri/src/ftp/server.rs`, `src-tauri/src/file_index/service.rs`, `src/components/PreviewWindow.tsx`, `src/components/PermissionList.tsx`, `src/components/ConfigCard.tsx`, `src/components/AdvancedConnectionConfig.tsx`, `src/stores/configStore.ts` |

**Estimated LOC reduction**: ~250 lines
**Estimated new LOC**: ~80 lines
**Net reduction**: ~170 lines
