# Dead Code & Simplification Cleanup — Stage 4

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove dead code and simplify redundant patterns across TypeScript, Rust, and Kotlin layers in 3 independent batches.

**Architecture:** Three batches by language, each independently buildable and verifiable. Each task follows TDD: write guard test → verify it fails → implement change → verify all tests pass → commit.

**Tech Stack:** TypeScript/Vitest, Rust/cargo, Kotlin/JUnit

---

## Batch 1: TypeScript

### Task 1: Inline `validatePort` into `parsePortInput` and remove `validation.ts`

**Files:**
- Delete: `src/utils/validation.ts`
- Modify: `src/hooks/usePortCheck.ts:9,20-38`
- Test: `src/hooks/__tests__/usePortCheck.test.ts` (existing tests should still pass)

- [ ] **Step 1: Write failing test that imports nothing from `validation.ts`**

Create `src/utils/__tests__/validation-removal-guard.test.ts`:

```typescript
/**
 * Guard: validation.ts has been inlined into usePortCheck.ts.
 * This test ensures no module re-introduces a dependency on validation.ts.
 */
import * as fs from 'fs';
import * as path from 'path';

test('no source file imports from utils/validation', () => {
  const srcRoot = path.resolve(__dirname, '..');
  const files = walkTsFiles(srcRoot);
  const violators: string[] = [];

  for (const file of files) {
    if (file.includes('validation-removal-guard')) continue;
    if (file.includes('node_modules')) continue;
    const content = fs.readFileSync(file, 'utf-8');
    if (content.includes("from '../utils/validation'") || content.includes("from '../../utils/validation'") || content.includes("from './validation'")) {
      violators.push(path.relative(srcRoot, file));
    }
  }

  expect(violators).toEqual([]);
});

function walkTsFiles(dir: string): string[] {
  const results: string[] = [];
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory() && entry.name !== 'node_modules') {
      results.push(...walkTsFiles(full));
    } else if (entry.isFile() && /\.(ts|tsx)$/.test(entry.name)) {
      results.push(full);
    }
  }
  return results;
}
```

- [ ] **Step 2: Run test to verify it fails (because `usePortCheck.ts` still imports from `validation`)**

Run: `npx vitest run src/utils/__tests__/validation-removal-guard.test.ts`
Expected: FAIL — `usePortCheck.ts` imports from `../utils/validation`

- [ ] **Step 3: Inline `validatePort` into `parsePortInput` and remove import**

In `src/hooks/usePortCheck.ts`, remove the import line and inline the validation:

Replace:
```typescript
import { validatePort } from '../utils/validation';
```
With: (remove entirely)

Replace the body of `parsePortInput`:
```typescript
export function parsePortInput(
  value: string,
  minPort: number,
  maxPort: number,
): PortSyntaxValidationResult {
  if (value.trim() === '') {
    return { valid: false, reason: 'empty' };
  }

  const port = parseInt(value, 10);
  if (isNaN(port) || port < 1 || port > 65535) {
    return { valid: false, reason: 'invalid_number' };
  }

  if (port < minPort || port > maxPort) {
    return { valid: false, reason: 'out_of_range' };
  }

  return { valid: true, port };
}
```

Then delete `src/utils/validation.ts`.

- [ ] **Step 4: Run all tests to verify they pass**

Run: `npx vitest run`
Expected: ALL PASS

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "refactor(ts): inline validatePort into parsePortInput, remove validation.ts"
```

---

### Task 2: Inline `AutoStartToggle` into `ConfigCard`

**Files:**
- Delete: `src/components/AutoStartToggle.tsx`
- Modify: `src/components/ConfigCard.tsx:13,17,146-151`

- [ ] **Step 1: Write failing test that no file imports `AutoStartToggle`**

Create `src/components/__tests__/autostart-toggle-removal-guard.test.ts`:

```typescript
import * as fs from 'fs';
import * as path from 'path';

test('no source file imports AutoStartToggle', () => {
  const srcRoot = path.resolve(__dirname, '..', '..');
  const files = walkTsFiles(srcRoot);
  const violators: string[] = [];

  for (const file of files) {
    if (file.includes('autostart-toggle-removal-guard')) continue;
    if (file.includes('node_modules')) continue;
    const content = fs.readFileSync(file, 'utf-8');
    if (content.includes("from './AutoStartToggle'") || content.includes("from '../AutoStartToggle'")) {
      violators.push(path.relative(srcRoot, file));
    }
  }

  expect(violators).toEqual([]);
});

function walkTsFiles(dir: string): string[] {
  const results: string[] = [];
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory() && entry.name !== 'node_modules') {
      results.push(...walkTsFiles(full));
    } else if (entry.isFile() && /\.(ts|tsx)$/.test(entry.name)) {
      results.push(full);
    }
  }
  return results;
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npx vitest run src/components/__tests__/autostart-toggle-removal-guard.test.ts`
Expected: FAIL — `ConfigCard.tsx` imports `AutoStartToggle`

- [ ] **Step 3: Inline `AutoStartToggle` usage in `ConfigCard.tsx`**

In `src/components/ConfigCard.tsx`:

1. Remove line 17: `import { AutoStartToggle } from './AutoStartToggle';`
2. Replace lines 147-151:
```tsx
            <AutoStartToggle
              enabled={autostartEnabled}
              onToggle={handleAutostartToggle}
            />
```
With:
```tsx
            <ToggleSwitch
              enabled={autostartEnabled}
              onChange={handleAutostartToggle}
              label="开机自启动"
              description="系统启动时自动运行图传伴侣"
            />
```

3. Delete `src/components/AutoStartToggle.tsx`

- [ ] **Step 4: Run all tests to verify they pass**

Run: `npx vitest run`
Expected: ALL PASS

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "refactor(ts): inline AutoStartToggle into ConfigCard"
```

---

### Task 3: Extract shared `usePreviewConfigListener` hook

**Files:**
- Create: `src/hooks/usePreviewConfigListener.ts`
- Modify: `src/components/PreviewWindow.tsx:93-106`
- Modify: `src/components/PreviewConfigCard.tsx:27-38`

- [ ] **Step 1: Write failing test for the new hook**

Create `src/hooks/__tests__/usePreviewConfigListener.test.ts`:

```typescript
import { describe, test, expect, vi, beforeEach } from 'vitest';

// The hook should re-export listen with proper typing for preview-config-changed
describe('usePreviewConfigListener', () => {
  test('module exports usePreviewConfigListener', async () => {
    const mod = await import('../usePreviewConfigListener');
    expect(typeof mod.usePreviewConfigListener).toBe('function');
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npx vitest run src/hooks/__tests__/usePreviewConfigListener.test.ts`
Expected: FAIL — module not found

- [ ] **Step 3: Create `src/hooks/usePreviewConfigListener.ts`**

```typescript
/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import type { ConfigChangedEvent } from '../types/events';

export function usePreviewConfigListener(
  callback: (config: ConfigChangedEvent['config']) => void,
  enabled = true,
) {
  useEffect(() => {
    if (!enabled) return;

    const unlistenPromise = listen<ConfigChangedEvent>('preview-config-changed', (event) => {
      callback(event.payload.config);
    });

    return () => {
      void unlistenPromise.then(fn => fn()).catch(() => {});
    };
  }, [enabled, callback]);
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `npx vitest run src/hooks/__tests__/usePreviewConfigListener.test.ts`
Expected: PASS

- [ ] **Step 5: Refactor `PreviewWindow.tsx` to use the new hook**

Replace lines 93-106 in `src/components/PreviewWindow.tsx`:
```tsx
  // 监听全局配置变化事件
  useEffect(() => {
    const setupListener = async () => {
      const unlisten = await listen<ConfigChangedEvent>('preview-config-changed', (event) => {
        setLocalAutoBringToFront(event.payload.config.autoBringToFront);
      });
      return unlisten;
    };

    const unlistenPromise = setupListener();
    return () => {
      void unlistenPromise.then(unlisten => unlisten()).catch(() => {});
    };
  }, []);
```

With:
```tsx
  usePreviewConfigListener(
    useCallback((config) => setLocalAutoBringToFront(config.autoBringToFront), []),
  );
```

Add imports at top (adjust existing imports):
```typescript
import { usePreviewConfigListener } from '../hooks/usePreviewConfigListener';
```

Remove unused `listen` import if no longer needed (check: `listen` is not used elsewhere in this file — it's only used in this block, so remove `import { listen } from '@tauri-apps/api/event';`).

- [ ] **Step 6: Refactor `PreviewConfigCard.tsx` to use the new hook**

Replace lines 27-38 in `src/components/PreviewConfigCard.tsx`:
```tsx
  // 监听全局配置变化事件
  useEffect(() => {
    if (!isWindows) return;

    const unlistenPromise = listen<ConfigChangedEvent>('preview-config-changed', (event) => {
      applyPreviewConfig(event.payload.config);
    });

    return () => {
      void unlistenPromise.then(fn => fn()).catch(() => {});
    };
  }, [isWindows, applyPreviewConfig]);
```

With:
```tsx
  usePreviewConfigListener(
    useCallback((config) => applyPreviewConfig(config), [applyPreviewConfig]),
    isWindows,
  );
```

Add imports:
```typescript
import { useCallback } from 'react';
import { usePreviewConfigListener } from '../hooks/usePreviewConfigListener';
```

Remove unused imports: `useEffect` (if no other useEffect in file — check: there are no other useEffect calls in PreviewConfigCard), `listen` from `@tauri-apps/api/event`, `ConfigChangedEvent` from `../types/events` (no longer directly referenced).

- [ ] **Step 7: Run all tests**

Run: `npx vitest run`
Expected: ALL PASS

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "refactor(ts): extract usePreviewConfigListener, deduplicate event listener"
```

---

## Batch 2: Rust

### Task 4: Remove dead `InsertResult` struct

**Files:**
- Modify: `src-tauri/src/ftp/android_mediastore/types.rs:40-47`
- Modify: `src-tauri/src/ftp/android_mediastore/mod.rs` (if re-exported)

- [ ] **Step 1: Write failing test asserting `InsertResult` does not exist**

Add to `src-tauri/src/ftp/android_mediastore/types.rs` tests module:

```rust
#[test]
fn insert_result_is_removed() {
    let source = include_str!("types.rs");
    assert!(
        !source.contains("pub struct InsertResult"),
        "InsertResult should be removed — it is never used"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo.exe test -p camera-ftp-companion-lib insert_result_is_removed -- --nocapture`
Expected: FAIL — assertion fails because `InsertResult` still exists

- [ ] **Step 3: Remove `InsertResult`**

In `src-tauri/src/ftp/android_mediastore/types.rs`, delete lines 40-47:
```rust
/// Result of a MediaStore insert operation.
#[derive(Debug, Clone)]
pub struct InsertResult {
    /// Content URI of the inserted file (e.g., content://media/external/images/media/123)
    pub content_uri: String,
    /// The display name of the file
    pub display_name: String,
}
```

Check `mod.rs` — `InsertResult` is NOT in the re-exports (line 56-59), so no change needed there.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo.exe test -p camera-ftp-companion-lib insert_result_is_removed`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "refactor(rust): remove unused InsertResult struct"
```

---

### Task 5: Remove dead `FtpAuthConfig::is_anonymous()` method

**Files:**
- Modify: `src-tauri/src/ftp/types.rs:63-68`

- [ ] **Step 1: Write failing test asserting `is_anonymous` does not exist**

Add to the tests module in `src-tauri/src/ftp/types.rs`:

```rust
#[test]
fn is_anonymous_method_is_removed() {
    let source = include_str!("types.rs");
    assert!(
        !source.contains("fn is_anonymous"),
        "is_anonymous() should be removed — it is never called"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo.exe test -p camera-ftp-companion-lib is_anonymous_method_is_removed`
Expected: FAIL

- [ ] **Step 3: Remove the method**

Delete lines 63-68 in `src-tauri/src/ftp/types.rs`:
```rust
impl FtpAuthConfig {
    /// 检查是否是匿名访问
    pub fn is_anonymous(&self) -> bool {
        matches!(self, Self::Anonymous)
    }
}
```

Keep the `impl FtpAuthConfig` block open if it has other methods (it doesn't currently — the `From<&AuthConfig>` is a separate impl block). Remove the entire empty `impl FtpAuthConfig {}` block.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo.exe test -p camera-ftp-companion-lib is_anonymous_method_is_removed`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "refactor(rust): remove unused FtpAuthConfig::is_anonymous()"
```

---

### Task 6: Remove dead `FtpServerHandle::get_snapshot()` and unused import

**Files:**
- Modify: `src-tauri/src/ftp/server.rs:130-135`
- Modify: `src-tauri/src/commands/server.rs:6`

- [ ] **Step 1: Write failing test asserting `get_snapshot` is removed from public API**

Add to `src-tauri/src/ftp/server.rs` tests (if module exists) or create guard test:

```rust
#[test]
fn get_snapshot_is_removed_from_handle() {
    let source = include_str!("server.rs");
    assert!(
        !source.contains("pub async fn get_snapshot"),
        "get_snapshot() should be removed from FtpServerHandle"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo.exe test -p camera-ftp-companion-lib get_snapshot_is_removed_from_handle`
Expected: FAIL

- [ ] **Step 3: Remove `get_snapshot()` method**

Delete lines 130-135 in `src-tauri/src/ftp/server.rs`:
```rust
    /// 获取状态快照
    pub async fn get_snapshot(&self) -> ServerStateSnapshot {
        self.send_command(|tx| ServerCommand::GetSnapshot { respond_to: tx })
            .await
            .unwrap_or_default()
    }
```

Also remove the unused `error` import from `src-tauri/src/commands/server.rs` line 6.

Change:
```rust
use tracing::{error, info, instrument};
```
To:
```rust
use tracing::{info, instrument};
```

Verify `error!` is no longer used in the file. Check: `error!` is used at line 117 in the `stop_server` function. So keep `error` in the import. **Correction:** `error` IS used at line 117. Do not remove it. Skip the unused import removal for `commands/server.rs`.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo.exe test -p camera-ftp-companion-lib get_snapshot_is_removed_from_handle`
Expected: PASS

- [ ] **Step 5: Verify full build**

Run: `./build.sh windows android`
Expected: Build succeeds

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "refactor(rust): remove unused FtpServerHandle::get_snapshot()"
```

---

### Task 7: Merge duplicate `#[cfg(test)]` methods in `backend.rs`

**Files:**
- Modify: `src-tauri/src/ftp/android_mediastore/backend.rs:138-265`

- [ ] **Step 1: Write failing test asserting no duplicate cfg(test) method pair exists**

Add to the existing test module in `backend.rs`:

```rust
#[test]
fn no_duplicate_cfg_test_method_pairs() {
    let source = include_str!("backend.rs");
    // Count occurrences of #[cfg(test)] followed by pub fn
    let cfg_test_pub_count = source.matches("#[cfg(test)]\n    pub fn").count();
    assert_eq!(
        cfg_test_pub_count, 0,
        "Found #[cfg(test)] pub fn duplicates — should use single pub(crate) implementation"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo.exe test -p camera-ftp-companion-lib no_duplicate_cfg_test_method_pairs`
Expected: FAIL — 3 duplicate pairs found

- [ ] **Step 3: Replace all 3 pairs with single `pub(crate)` implementations**

In `src-tauri/src/ftp/android_mediastore/backend.rs`, replace lines 138-265 with:

```rust
    /// Normalizes a path by removing leading slashes and resolving "." and "..".
    pub(crate) fn normalize_path(&self, path: &Path) -> PathBuf {
        let path_str = path.to_string_lossy();
        let normalized = path_str.trim_start_matches('/');
        
        let mut components = Vec::new();
        for part in normalized.split('/') {
            match part {
                "" | "." => {}
                ".." => {
                    components.pop();
                }
                _ => components.push(part),
            }
        }
        
        PathBuf::from(components.join("/"))
    }

    /// Resolves a user-provided path to the full relative path in MediaStore.
    pub(crate) fn resolve_path(&self, path: &Path) -> String {
        let normalized = self.normalize_path(path);
        
        let full_path = if normalized.starts_with("DCIM/") || normalized.starts_with("Pictures/") {
            normalized.to_string_lossy().to_string()
        } else {
            format!("{}{}", self.base_relative_path, normalized.to_string_lossy())
        };
        
        full_path
    }

    /// Validates a path for security (prevents directory traversal attacks).
    pub(crate) fn validate_path(&self, path: &Path) -> Result<(), StorageError> {
        let path_str = path.to_string_lossy();
        
        if path_str.contains('\0') {
            return Err(StorageError::from(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Path contains null bytes",
            )));
        }
        
        if path_str.contains("..") {
            let normalized = self.normalize_path(path);
            if normalized.to_string_lossy().contains("..") {
                return Err(StorageError::from(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "Path traversal attempt detected",
                )));
            }
        }
        
        Ok(())
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo.exe test -p camera-ftp-companion-lib no_duplicate_cfg_test_method_pairs`
Expected: PASS

- [ ] **Step 5: Verify all backend tests still pass**

Run: `cargo.exe test -p camera-ftp-companion-lib -- android_mediastore`
Expected: ALL PASS

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "refactor(rust): merge duplicate cfg(test) method pairs in backend.rs"
```

---

### Task 8: Extract duplicated auth credential extraction to `FtpAuthConfig` method

**Files:**
- Modify: `src-tauri/src/ftp/types.rs` (add method to `FtpAuthConfig`)
- Modify: `src-tauri/src/commands/server.rs:69-80`
- Modify: `src-tauri/src/ftp/server.rs:635-647`

- [ ] **Step 1: Write failing test for `FtpAuthConfig::to_display_credentials()`**

Add to the tests module in `src-tauri/src/ftp/types.rs`:

```rust
#[test]
fn ftp_auth_config_to_display_credentials() {
    use super::FtpAuthConfig;

    let anonymous = FtpAuthConfig::Anonymous;
    assert_eq!(anonymous.to_display_credentials(), (None, None));

    let authed = FtpAuthConfig::Authenticated {
        username: "admin".to_string(),
        password_hash: "hash123".to_string(),
    };
    let (user, pass_info) = authed.to_display_credentials();
    assert_eq!(user.as_deref(), Some("admin"));
    assert_eq!(pass_info.as_deref(), Some("(配置密码)"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo.exe test -p camera-ftp-companion-lib ftp_auth_config_to_display_credentials`
Expected: FAIL — method does not exist

- [ ] **Step 3: Add `to_display_credentials()` to `FtpAuthConfig`**

Add a new impl block in `src-tauri/src/ftp/types.rs`:

```rust
impl FtpAuthConfig {
    /// Returns (username, password_info) suitable for display in UI.
    /// Anonymous → (None, None), Authenticated → (Some(username), Some("(配置密码)"))
    pub fn to_display_credentials(&self) -> (Option<String>, Option<String>) {
        match self {
            Self::Anonymous => (None, None),
            Self::Authenticated { username, .. } => {
                (Some(username.clone()), Some("(配置密码)".to_string()))
            }
        }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo.exe test -p camera-ftp-companion-lib ftp_auth_config_to_display_credentials`
Expected: PASS

- [ ] **Step 5: Refactor `commands/server.rs` to use new method**

Replace lines 69-80 in `src-tauri/src/commands/server.rs`:

From:
```rust
    let (username, password_info) = if app_config.advanced_connection.enabled {
        if app_config.advanced_connection.auth.anonymous {
            (None, None)
        } else {
            (
                Some(app_config.advanced_connection.auth.username),
                Some("(配置密码)".to_string()),
            )
        }
    } else {
        (None, None)
    };
```

To:
```rust
    let auth_config = if app_config.advanced_connection.enabled {
        crate::ftp::types::FtpAuthConfig::from(&app_config.advanced_connection.auth)
    } else {
        crate::ftp::types::FtpAuthConfig::Anonymous
    };
    let (username, password_info) = auth_config.to_display_credentials();
```

- [ ] **Step 6: Refactor `ftp/server.rs` to use new method**

Replace lines 635-647 in `src-tauri/src/ftp/server.rs`:

From:
```rust
        let (username, password_info) = if let Some(ref config) = self.config {
            match &config.auth {
                FtpAuthConfig::Anonymous => (None, None),
                FtpAuthConfig::Authenticated { username, .. } => {
                    (
                        Some(username.clone()),
                        Some("(配置密码)".to_string()),
                    )
                }
            }
        } else {
            (None, None)
        };
```

To:
```rust
        let (username, password_info) = self.config
            .as_ref()
            .map(|c| c.auth.to_display_credentials())
            .unwrap_or((None, None));
```

- [ ] **Step 7: Verify all tests pass**

Run: `./build.sh windows android`
Expected: Build succeeds

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "refactor(rust): extract auth credential display logic to FtpAuthConfig::to_display_credentials"
```

---

### Task 9: Inline thin free functions in `platform/android.rs`

**Files:**
- Modify: `src-tauri/src/platform/android.rs:32-53,56-61,115-127,148-158`

- [ ] **Step 1: Write failing test asserting no redundant public free functions**

Add to the tests module in `src-tauri/src/platform/android.rs`:

```rust
#[test]
fn no_redundant_storage_free_functions() {
    let source = include_str!("android.rs");
    // These free functions should be inlined into the trait impl
    assert!(
        !source.contains("pub fn get_storage_info()"),
        "get_storage_info() should be inlined into PlatformService impl"
    );
    assert!(
        !source.contains("pub fn check_permission_status()"),
        "check_permission_status() should be inlined into PlatformService impl"
    );
    assert!(
        !source.contains("pub fn ensure_storage_ready()"),
        "ensure_storage_ready() should be inlined into PlatformService impl"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo.exe test -p camera-ftp-companion-lib no_redundant_storage_free_functions`
Expected: FAIL

- [ ] **Step 3: Inline the functions into the trait impl**

In `src-tauri/src/platform/android.rs`:

1. Delete the three public free functions: `get_storage_info()` (lines 32-53), `check_permission_status()` (lines 56-61), `ensure_storage_ready()` (lines 115-127).

2. Keep the private helpers: `can_write_to_dcim()` and `validate_path_writable()` (they are used by the inlined logic).

3. Update the trait impl to inline the logic:

```rust
    fn get_storage_info(&self) -> StorageInfo {
        let path = DEFAULT_STORAGE_PATH;
        let path_buf = std::path::PathBuf::from(path);

        let exists = path_buf.exists();
        let writable = if exists {
            validate_path_writable(path)
        } else {
            false
        };

        let has_all_files_access = writable || (exists && can_write_to_dcim());

        StorageInfo {
            display_name: STORAGE_DISPLAY_NAME.to_string(),
            path: path.to_string(),
            exists,
            writable,
            has_all_files_access,
        }
    }

    fn check_permission_status(&self) -> PermissionStatus {
        PermissionStatus {
            has_all_files_access: true,
            needs_user_action: false,
        }
    }

    fn ensure_storage_ready(&self, _app: &AppHandle) -> Result<String, String> {
        let path = DEFAULT_STORAGE_PATH;
        let path_buf = std::path::PathBuf::from(path);

        if !path_buf.exists() {
            std::fs::create_dir_all(&path_buf).map_err(|e| format!("无法创建存储目录: {}", e))?;
            info!("Created storage directory: {}", path);
        }

        Ok(path.to_string())
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo.exe test -p camera-ftp-companion-lib no_redundant_storage_free_functions`
Expected: PASS

- [ ] **Step 5: Verify full build**

Run: `./build.sh windows android`
Expected: Build succeeds

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "refactor(rust): inline thin free functions into AndroidPlatform trait impl"
```

---

## Batch 3: Kotlin

### Task 10: Remove dead `deleteDiskEntries()` from `ThumbnailCacheV2`

**Files:**
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/galleryv2/ThumbnailCacheV2.kt:182-191`

- [ ] **Step 1: Write failing test asserting `deleteDiskEntries` is removed**

Add to an existing test file or create a new guard test in `src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/galleryv2/ThumbnailCacheV2DeadCodeTest.kt`:

```kotlin
package com.gjk.cameraftpcompanion.galleryv2

import org.junit.Assert.*
import org.junit.Test

class ThumbnailCacheV2DeadCodeTest {
    @Test
    fun `deleteDiskEntries is removed`() {
        val source = ThumbnailCacheV2::class.java
            .getResourceAsStream("ThumbnailCacheV2.class")
        // Simpler: check the method doesn't exist via reflection
        val methods = ThumbnailCacheV2::class.java.declaredMethods
        val hasMethod = methods.any { it.name == "deleteDiskEntries" }
        assertFalse("deleteDiskEntries should be removed — it is never called", hasMethod)
    }
}
```

Actually, a simpler approach — since this is a `private` method, reflection won't easily find it without `isAccessible = true`. Use a source-based guard instead:

```kotlin
package com.gjk.cameraftpcompanion.galleryv2

import org.junit.Assert.*
import org.junit.Test
import java.io.File

class ThumbnailCacheV2DeadCodeTest {
    @Test
    fun `deleteDiskEntries is removed`() {
        val sourceFile = File(
            "src/main/java/com/gjk/cameraftpcompanion/galleryv2/ThumbnailCacheV2.kt"
        )
        val relativePath = if (sourceFile.exists()) sourceFile
            else File("app/src/main/java/com/gjk/cameraftpcompanion/galleryv2/ThumbnailCacheV2.kt")
        val source = relativePath.readText()
        assertFalse(
            "deleteDiskEntries should be removed — it is never called",
            source.contains("fun deleteDiskEntries")
        )
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run Android tests or build to verify
Expected: FAIL

- [ ] **Step 3: Delete `deleteDiskEntries` method**

Remove lines 182-191 in `ThumbnailCacheV2.kt`:
```kotlin
    private fun deleteDiskEntries(key: String) {
        val root = cacheRoot ?: return
        root.walkTopDown()
            .filter { it.isFile && it.nameWithoutExtension.endsWith("_$key") }
            .forEach {
                if (it.delete()) {
                    Log.d(TAG, "Invalidated disk entry: ${it.name}")
                }
            }
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: Android build
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "refactor(kotlin): remove unused deleteDiskEntries from ThumbnailCacheV2"
```

---

### Task 11: Remove dead `shouldRequestDeleteConfirmation(apiLevel, throwable)` overload

**Files:**
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/GalleryBridge.kt:43-50`

- [ ] **Step 1: Write failing test asserting the overload is removed**

Create `src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/bridges/GalleryBridgeDeadOverloadTest.kt`:

```kotlin
package com.gjk.cameraftpcompanion.bridges

import org.junit.Assert.*
import org.junit.Test
import java.io.File

class GalleryBridgeDeadOverloadTest {
    @Test
    fun `throwable overload of shouldRequestDeleteConfirmation is removed`() {
        val sourceFile = File(
            "src/main/java/com/gjk/cameraftpcompanion/bridges/GalleryBridge.kt"
        )
        val relativePath = if (sourceFile.exists()) sourceFile
            else File("app/src/main/java/com/gjk/cameraftpcompanion/bridges/GalleryBridge.kt")
        val source = relativePath.readText()
        assertFalse(
            "shouldRequestDeleteConfirmation(apiLevel, throwable) overload should be removed",
            source.contains("throwable: Throwable)")
        )
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Expected: FAIL

- [ ] **Step 3: Delete the overload**

Remove lines 43-50 in `GalleryBridge.kt`:
```kotlin
        @JvmStatic
        fun shouldRequestDeleteConfirmation(apiLevel: Int, throwable: Throwable): Boolean {
            return shouldRequestDeleteConfirmation(
                apiLevel = apiLevel,
                isSecurityException = throwable is SecurityException,
                isRecoverableSecurityException = throwable is RecoverableSecurityException,
            )
        }
```

- [ ] **Step 4: Run test to verify it passes**

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "refactor(kotlin): remove unused shouldRequestDeleteConfirmation overload"
```

---

### Task 12: Remove `openViewer` delegate from `ImageViewerBridge` and simplify TS side

**Files:**
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/ImageViewerBridge.kt:22-26`
- Modify: `src/services/image-open.ts:76-96`
- Modify: `src/types/global.ts:174-175`

- [ ] **Step 1: Write failing test asserting `openViewer` is removed from bridge**

Create `src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/bridges/ImageViewerBridgeDeadCodeTest.kt`:

```kotlin
package com.gjk.cameraftpcompanion.bridges

import org.junit.Assert.*
import org.junit.Test
import java.io.File

class ImageViewerBridgeDeadCodeTest {
    @Test
    fun `openViewer delegate is removed`() {
        val sourceFile = File(
            "src/main/java/com/gjk/cameraftpcompanion/bridges/ImageViewerBridge.kt"
        )
        val relativePath = if (sourceFile.exists()) sourceFile
            else File("app/src/main/java/com/gjk/cameraftpcompanion/bridges/ImageViewerBridge.kt")
        val source = relativePath.readText()
        assertFalse(
            "openViewer() delegate should be removed — use openOrNavigateTo directly",
            source.contains("fun openViewer(uri: String, allUrisJson: String)")
        )
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Expected: FAIL

- [ ] **Step 3: Remove `openViewer` from `ImageViewerBridge.kt`**

Delete lines 22-26:
```kotlin
    @android.webkit.JavascriptInterface
    fun openViewer(uri: String, allUrisJson: String): Boolean {
        Log.d(TAG, "openViewer: uri=$uri")
        return openOrNavigateTo(uri, allUrisJson)
    }
```

- [ ] **Step 4: Update `image-open.ts` to remove `openViewer` call path**

In `src/services/image-open.ts`, replace lines 76-96:

From:
```typescript
    if (preferReuse && imageViewerAndroid.openOrNavigateTo) {
      try {
        if (imageViewerAndroid.openOrNavigateTo(filePath, viewerUrisJson)) {
          void sendExifToViewer(filePath);
          return;
        }
      } catch {
        // Fall through to other open methods when bridge call fails.
      }
    }

    if (imageViewerAndroid.openViewer) {
      try {
        if (imageViewerAndroid.openViewer(filePath, viewerUrisJson)) {
          void sendExifToViewer(filePath);
          return;
        }
      } catch {
        // Fall through to chooser/window fallback when bridge call fails.
      }
    }
```

To:
```typescript
    if (imageViewerAndroid.openOrNavigateTo) {
      try {
        if (imageViewerAndroid.openOrNavigateTo(filePath, viewerUrisJson)) {
          void sendExifToViewer(filePath);
          return;
        }
      } catch {
        // Fall through to chooser/window fallback when bridge call fails.
      }
    }
```

Note: The `preferReuse` guard is removed since `openOrNavigateTo` already handles reuse internally. This simplifies the logic.

- [ ] **Step 5: Update `global.ts` to remove `openViewer` from interface**

In `src/types/global.ts`, remove lines 174-175:
```typescript
  openViewer(uri: string, allUrisJson: string): boolean;
```

And update the interface comment if needed.

- [ ] **Step 6: Run all TS tests**

Run: `npx vitest run`
Expected: ALL PASS

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "refactor: remove openViewer delegate, unify on openOrNavigateTo"
```

---

### Task 13: Remove write-only `totalRequests`/`cacheHits` counters and simplify `PermissionBridge` Toast usage

**Files:**
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/galleryv2/ThumbnailPipelineManager.kt:138-141,220-230,418`
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/GalleryBridgeV2.kt:126`
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/PermissionBridge.kt:271,308,322,336`
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/GalleryBridgeV2.kt:9`

- [ ] **Step 1: Write failing tests**

Create `src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/galleryv2/ThumbnailPipelineDeadCodeTest.kt`:

```kotlin
package com.gjk.cameraftpcompanion.galleryv2

import org.junit.Assert.*
import org.junit.Test
import java.io.File

class ThumbnailPipelineDeadCodeTest {
    @Test
    fun `write-only counters are removed`() {
        val sourceFile = resolveSourceFile(
            "src/main/java/com/gjk/cameraftpcompanion/galleryv2/ThumbnailPipelineManager.kt"
        )
        val source = sourceFile.readText()
        assertFalse(
            "totalRequests write-only counter should be removed",
            source.contains("private var totalRequests")
        )
        assertFalse(
            "cacheHits write-only counter should be removed",
            source.contains("private var cacheHits")
        )
    }

    private fun resolveSourceFile(relativePath: String): File {
        val candidates = listOf(File(relativePath), File("app/$relativePath"))
        return candidates.first { it.exists() }
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Expected: FAIL

- [ ] **Step 3: Remove write-only counters from `ThumbnailPipelineManager.kt`**

1. Delete lines 138-141:
```kotlin
    // ── Cache hit tracking ─────────────────────────────────────────────

    private var totalRequests: Long = 0
    private var cacheHits: Long = 0
```

2. Delete the `recordCacheHit()` method (lines 216-223):
```kotlin
    fun recordCacheHit() = lock.withLock {
        totalRequests++
        cacheHits++
    }
```

3. Delete the `recordCacheMiss()` method (lines 225-230):
```kotlin
    fun recordCacheMiss() = lock.withLock {
        totalRequests++
    }
```

4. Remove the call to `recordCacheMiss()` at line 418.

5. In `GalleryBridgeV2.kt`, remove the call to `pipelineManager.recordCacheHit()` at line 126. Also remove the local `cacheHits` variable at line 113 and the increment at line 125. Update the log line at 155 to remove `cacheHits` reference.

- [ ] **Step 4: Fix `PermissionBridge.kt` Toast usage**

In `PermissionBridge.kt`, replace all fully-qualified `android.widget.Toast.makeText(...)` with `Toast.makeText(...)` at lines 271, 308, 322, 336. The import `android.widget.Toast` is already present at line 20.

- [ ] **Step 5: Remove unused import in `GalleryBridgeV2.kt`**

Remove line 9: `import android.app.Activity` (unused).

- [ ] **Step 6: Run tests**

Run: Android build + existing tests
Expected: ALL PASS

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "refactor(kotlin): remove write-only counters, simplify Toast, remove unused import"
```

---

### Task 14: Verify full build across all platforms

**Files:** None (verification only)

- [ ] **Step 1: Run full build**

Run: `./build.sh windows android`
Expected: Both platforms build successfully

- [ ] **Step 2: Run all TS tests**

Run: `npx vitest run`
Expected: ALL PASS
