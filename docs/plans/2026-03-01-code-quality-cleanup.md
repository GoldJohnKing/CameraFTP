# Code Quality Cleanup Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Remove dead code, consolidate duplicates, fix performance issues, and fix debounce bug.

**Architecture:** Incremental cleanup with verification after each task. No feature changes - only code quality improvements.

**Tech Stack:** TypeScript/React, Rust (Tauri), Kotlin (Android)

---

## Task 1: Delete Unused PortSelector Component

**Files:**
- Delete: `src/components/PortSelector.tsx`

**Step 1: Verify no imports exist**

Run: `grep -r "PortSelector" src/`
Expected: No results (file only exports, never imported)

**Step 2: Delete the file**

```bash
rm src/components/PortSelector.tsx
```

**Step 3: Verify build passes**

Run: `./build.sh frontend`
Expected: Build succeeds

**Step 4: Commit**

```bash
git add -A
git commit -m "refactor: remove unused PortSelector component"
```

---

## Task 2: Remove Unused npm Dependencies

**Files:**
- Modify: `package.json`

**Step 1: Remove unused dependencies**

In `package.json`, remove these lines from `dependencies`:
```json
"@tauri-apps/plugin-process": "^2.0.0",
"@tauri-apps/plugin-shell": "^2.0.0",
```

**Step 2: Install to update lockfile**

Run: `npm install`
Expected: Dependencies removed from package-lock.json

**Step 3: Verify build passes**

Run: `./build.sh frontend`
Expected: Build succeeds

**Step 4: Commit**

```bash
git add package.json package-lock.json
git commit -m "refactor: remove unused tauri plugin dependencies"
```

---

## Task 3: Remove Dead Rust Method

**Files:**
- Modify: `src-tauri/src/ftp/listeners.rs`

**Step 1: Find and remove the dead method**

In `src-tauri/src/ftp/listeners.rs`, remove the `set_app_handle` method from `FtpDataListener`:

```rust
// DELETE THIS METHOD (approximately line 24):
pub fn set_app_handle(&mut self, handle: AppHandle) {
    self.app_handle = Some(handle);
}
```

**Step 2: Verify build passes**

Run: `./build.sh windows`
Expected: Build succeeds

**Step 3: Commit**

```bash
git add src-tauri/src/ftp/listeners.rs
git commit -m "refactor: remove unused set_app_handle method"
```

---

## Task 4: Fix Debounce Singleton Closure Bug

**Files:**
- Modify: `src/stores/configStore.ts`

**Step 1: Read current implementation**

Run: Read `src/stores/configStore.ts` lines 70-95

**Step 2: Replace debounce implementation**

Find this code (around lines 76-87):
```typescript
let debouncedSave: ReturnType<typeof debounce> | null = null;

const getOrCreateDebouncedSave = (saveFn: (config: AppConfig) => Promise<void>) => {
  if (!debouncedSave) {
    debouncedSave = debounce(async (config: AppConfig) => {
      await saveFn(config);
    }, DEBOUNCE_DELAY);
  }
  return debouncedSave;
};
```

Replace with:
```typescript
const createDebouncedSave = (saveFn: (config: AppConfig) => Promise<void>) => {
  return debounce(async (config: AppConfig) => {
    await saveFn(config);
  }, DEBOUNCE_DELAY);
};
```

**Step 3: Update usage**

Find where `getOrCreateDebouncedSave` is called (in the store definition) and replace:
```typescript
// OLD:
const debouncedSave = getOrCreateDebouncedSave(saveConfig);

// NEW:
const debouncedSave = createDebouncedSave(saveConfig);
```

**Step 4: Verify build passes**

Run: `./build.sh frontend`
Expected: Build succeeds

**Step 5: Commit**

```bash
git add src/stores/configStore.ts
git commit -m "fix: debounce closure captures fresh saveFn reference"
```

---

## Task 5: Extract Shared Port Validation Utility

**Files:**
- Create: `src/utils/validation.ts`
- Modify: `src/components/AdvancedConnectionConfig.tsx`

**Step 1: Create validation utility**

Create `src/utils/validation.ts`:
```typescript
/**
 * Validates a port string and returns the parsed port number.
 * Returns null if invalid (non-numeric, < 1, or > 65535).
 */
export function validatePort(value: string): number | null {
  const port = parseInt(value, 10);
  if (isNaN(port) || port < 1 || port > 65535) {
    return null;
  }
  return port;
}
```

**Step 2: Update AdvancedConnectionConfig to use shared utility**

In `src/components/AdvancedConnectionConfig.tsx`:

Add import at top:
```typescript
import { validatePort } from '../utils/validation';
```

Find and replace the local `validatePort` function (around lines 93-109) with a call to the imported function.

The local function looks like:
```typescript
const validatePort = (value: string): number | null => {
  const port = parseInt(value, 10);
  if (isNaN(port) || port < 1 || port > 65535) {
    return null;
  }
  return port;
};
```

Delete this local function - the imported one will be used instead.

**Step 3: Verify build passes**

Run: `./build.sh frontend`
Expected: Build succeeds

**Step 4: Commit**

```bash
git add src/utils/validation.ts src/components/AdvancedConnectionConfig.tsx
git commit -m "refactor: extract shared validatePort utility"
```

---

## Task 6: Create usePortCheck Hook

**Files:**
- Create: `src/hooks/usePortCheck.ts`
- Modify: `src/components/AdvancedConnectionConfig.tsx`

**Step 1: Create the hook**

Create `src/hooks/usePortCheck.ts`:
```typescript
import { useState, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { validatePort } from './validation';

interface UsePortCheckResult {
  checkPort: (value: string) => Promise<{ valid: boolean; available: boolean }>;
  isChecking: boolean;
  portError: string | null;
  clearError: () => void;
}

export function usePortCheck(): UsePortCheckResult {
  const [isChecking, setIsChecking] = useState(false);
  const [portError, setPortError] = useState<string | null>(null);

  const checkPort = useCallback(async (value: string) => {
    const port = validatePort(value);
    if (port === null) {
      setPortError('Port must be between 1 and 65535');
      return { valid: false, available: false };
    }

    setIsChecking(true);
    setPortError(null);

    try {
      const available = await invoke<boolean>('check_port_available', { port });
      if (!available) {
        setPortError('Port is already in use');
      }
      return { valid: true, available };
    } catch (e) {
      setPortError('Failed to check port availability');
      return { valid: true, available: false };
    } finally {
      setIsChecking(false);
    }
  }, []);

  const clearError = useCallback(() => {
    setPortError(null);
  }, []);

  return { checkPort, isChecking, portError, clearError };
}
```

**Step 2: Update AdvancedConnectionConfig to use the hook**

In `src/components/AdvancedConnectionConfig.tsx`:

Add import:
```typescript
import { usePortCheck } from '../hooks/usePortCheck';
```

In the component, replace the manual state and handler with the hook:
```typescript
// Replace:
const [isCheckingPort, setIsCheckingPort] = useState(false);
const [portError, setPortError] = useState<string | null>(null);

// With:
const { checkPort, isChecking: isCheckingPort, portError, clearError } = usePortCheck();
```

Update the `handlePortBlur` handler to use `checkPort` from the hook.

**Step 3: Verify build passes**

Run: `./build.sh frontend`
Expected: Build succeeds

**Step 4: Commit**

```bash
git add src/hooks/usePortCheck.ts src/components/AdvancedConnectionConfig.tsx
git commit -m "refactor: extract usePortCheck hook for port validation"
```

---

## Task 7: Consolidate Rust Server Command Pattern

**Files:**
- Modify: `src-tauri/src/ftp/server.rs`

**Step 1: Add helper method to FtpServerHandle**

In `src-tauri/src/ftp/server.rs`, add this helper method to `FtpServerHandle`:

```rust
async fn send_command<T: Send + 'static>(
    &self,
    cmd_factory: impl FnOnce(oneshot::Sender<T>) -> ServerCommand,
) -> Result<T, AppError> {
    let (tx, rx) = oneshot::channel();
    let cmd = cmd_factory(tx);
    if self.tx.send(cmd).await.is_err() {
        return Err(AppError::ServerNotRunning);
    }
    rx.await.map_err(|_| AppError::ServerNotRunning)
}
```

**Step 2: Refactor existing methods to use helper**

Update methods like `get_server_info`, `start`, `stop` to use the helper. For example:

```rust
// Before:
pub async fn get_server_info(&self) -> Option<ServerInfo> {
    let (tx, rx) = oneshot::channel();
    if self.tx.send(ServerCommand::GetInfo { response: tx }).await.is_err() {
        return None;
    }
    rx.await.ok().flatten()
}

// After:
pub async fn get_server_info(&self) -> Option<ServerInfo> {
    self.send_command(|tx| ServerCommand::GetInfo { response: tx })
        .await
        .ok()
        .flatten()
}
```

Note: The exact refactoring depends on the `ServerCommand` enum variants. Adjust as needed.

**Step 3: Verify build passes**

Run: `./build.sh windows`
Expected: Build succeeds

**Step 4: Commit**

```bash
git add src-tauri/src/ftp/server.rs
git commit -m "refactor: add send_command helper to reduce boilerplate"
```

---

## Task 8: Use Arc<PathBuf> in FtpDataListener

**Files:**
- Modify: `src-tauri/src/ftp/listeners.rs`
- Modify: `src-tauri/src/ftp/server.rs` (where listener is created)

**Step 1: Update FtpDataListener struct**

In `src-tauri/src/ftp/listeners.rs`, change the struct field:

```rust
// Before:
pub save_path: std::path::PathBuf,

// After:
pub save_path: std::sync::Arc<std::path::PathBuf>,
```

**Step 2: Update usages**

Find all places where `self.save_path` is used and update:
- `path.clone()` becomes `Arc::clone(&self.save_path)` or just `(*self.save_path).clone()` if PathBuf is needed

**Step 3: Update creation site**

In `src-tauri/src/ftp/server.rs` or wherever `FtpDataListener` is created, wrap the PathBuf in Arc:
```rust
FtpDataListener {
    save_path: Arc::new(save_path),
    // ...
}
```

**Step 4: Verify build passes**

Run: `./build.sh windows`
Expected: Build succeeds

**Step 5: Commit**

```bash
git add src-tauri/src/ftp/listeners.rs src-tauri/src/ftp/server.rs
git commit -m "perf: use Arc<PathBuf> to avoid cloning on every FTP event"
```

---

## Task 9: Add memo() to PreviewWindowContent

**Files:**
- Modify: `src/components/PreviewWindow.tsx`

**Step 1: Import memo**

Add `memo` to the React import:
```typescript
import { useState, useEffect, useRef, useCallback, useMemo, memo } from 'react';
```

**Step 2: Wrap PreviewWindowContent with memo**

Find the `PreviewWindowContent` component definition and wrap it:
```typescript
// Before:
function PreviewWindowContent(props: PreviewWindowContentProps) {
  // ...
}

// After:
const PreviewWindowContent = memo(function PreviewWindowContent(props: PreviewWindowContentProps) {
  // ...
});
```

**Step 3: Memoize loadExifInfo with useCallback**

Find the `loadExifInfo` function definition and wrap with useCallback:
```typescript
const loadExifInfo = useCallback(async () => {
  // existing implementation
}, [imagePath]); // Add imagePath as dependency
```

**Step 4: Memoize filename extraction**

Find where filename is extracted from path and memoize:
```typescript
const fileName = useMemo(() => {
  return imagePath ? imagePath.split(/[/\\]/).pop() || '' : '';
}, [imagePath]);
```

**Step 5: Verify build passes**

Run: `./build.sh frontend`
Expected: Build succeeds

**Step 6: Commit**

```bash
git add src/components/PreviewWindow.tsx
git commit -m "perf: add memo and useCallback to PreviewWindowContent"
```

---

## Task 10: Move PERMISSION_CONFIGS Outside Component

**Files:**
- Modify: `src/components/PermissionList.tsx`

**Step 1: Find PERMISSION_CONFIGS**

Locate the `PERMISSION_CONFIGS` array inside the component (around line 113).

**Step 2: Move it outside the component**

Cut the entire `PERMISSION_CONFIGS` definition and paste it above the component function, at module level:

```typescript
// Move to top of file, after imports
const PERMISSION_CONFIGS: PermissionConfig[] = [
  // ... existing array contents
];

// Component starts here
export function PermissionList() {
  // ...
}
```

**Step 3: Verify build passes**

Run: `./build.sh frontend`
Expected: Build succeeds

**Step 4: Commit**

```bash
git add src/components/PermissionList.tsx
git commit -m "perf: move PERMISSION_CONFIGS outside component to prevent recreation"
```

---

## Task 11: Memoize Default Config in ConfigCard

**Files:**
- Modify: `src/components/ConfigCard.tsx`

**Step 1: Find the inline object**

Locate the inline default config object (around line 173):
```typescript
const currentConfig = draft?.advancedConnection ?? {
  enabled: false,
  auth: { anonymous: true, username: '', password: '' },
  pasv: { portStart: 50000, portEnd: 50100 }
};
```

**Step 2: Extract to module-level constant**

Add at top of file (after imports):
```typescript
const DEFAULT_ADVANCED_CONFIG: AdvancedConnectionConfig = {
  enabled: false,
  auth: { anonymous: true, username: '', password: '' },
  pasv: { portStart: 50000, portEnd: 50100 }
};
```

**Step 3: Update usage**

Replace inline object with constant:
```typescript
const currentConfig = draft?.advancedConnection ?? DEFAULT_ADVANCED_CONFIG;
```

**Step 4: Verify build passes**

Run: `./build.sh frontend`
Expected: Build succeeds

**Step 5: Commit**

```bash
git add src/components/ConfigCard.tsx
git commit -m "perf: extract default config to module-level constant"
```

---

## Task 12: Final Verification

**Step 1: Full build verification**

Run:
```bash
./build.sh frontend && ./build.sh windows && ./build.sh android
```
Expected: All builds succeed

**Step 2: Manual smoke test**

- Start the app on Windows
- Verify server start/stop works
- Navigate through config tabs
- Check Android app starts

**Step 3: Final commit (if any cleanup needed)**

```bash
git add -A
git commit -m "chore: final cleanup after code quality improvements"
```

---

## Summary

| Task | Description | Files Changed |
|------|-------------|---------------|
| 1 | Delete PortSelector | 1 deleted |
| 2 | Remove unused deps | package.json |
| 3 | Remove dead Rust method | listeners.rs |
| 4 | Fix debounce bug | configStore.ts |
| 5 | Extract validatePort | 1 created, 1 modified |
| 6 | Create usePortCheck | 1 created, 1 modified |
| 7 | Consolidate server commands | server.rs |
| 8 | Arc<PathBuf> optimization | 2 modified |
| 9 | memo() PreviewWindow | PreviewWindow.tsx |
| 10 | Move PERMISSION_CONFIGS | PermissionList.tsx |
| 11 | Extract default config | ConfigCard.tsx |
| 12 | Final verification | - |
