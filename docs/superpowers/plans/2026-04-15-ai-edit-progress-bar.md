# AI 修图进度条 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace all AI edit Toast notifications with an animated progress bar showing "第 X 张 / 共 N 张" text, driven by backend progress events.

**Architecture:** Backend `AiEditService` worker_loop emits `ai-edit-progress` Tauri events for each task start/complete/fail and queue-done. Frontend listens via `listen('ai-edit-progress')` and updates a shared zustand store. A single `AiEditProgressBar` React component renders the progress bar with different positioning per scene. Android Native gets its own progress bar UI synced via JS bridge.

**Tech Stack:** Rust (tauri, ts-rs, serde, tokio), TypeScript/React (zustand, @tauri-apps/api/event, tailwindcss), Kotlin (Android Native UI)

**Spec:** `docs/superpowers/specs/2026-04-15-ai-edit-progress-bar-design.md`

---

## File Structure

### Backend (Rust) — Create/Modify

| File | Responsibility |
|------|---------------|
| `src-tauri/src/ai_edit/progress.rs` | **NEW** — `AiEditProgressEvent` enum with ts-rs export |
| `src-tauri/src/ai_edit/mod.rs` | Add `pub mod progress;` export |
| `src-tauri/src/ai_edit/service.rs` | Add progress emission in worker_loop; add `enqueue_manual`, `queue_len`, `cancel` methods |
| `src-tauri/src/commands/ai_edit.rs` | Add `enqueue_ai_edit`, `cancel_ai_edit` commands |
| `src-tauri/src/commands/mod.rs` | Re-export new commands |
| `src-tauri/src/lib.rs` | Register new commands in `invoke_handler![]` |

### Frontend (TypeScript/React) — Create/Modify

| File | Responsibility |
|------|---------------|
| `src/hooks/useAiEditProgress.ts` | **NEW** — Zustand store + hook, listens to `ai-edit-progress` events |
| `src/components/AiEditProgressBar.tsx` | **NEW** — Reusable progress bar UI component |
| `src/types/index.ts` | Add re-export for `AiEditProgressEvent` type |
| `src/components/PreviewWindow.tsx` | Remove `aiEditing` state, use global progress, render progress bar |
| `src/hooks/useGallerySelection.ts` | Remove toast, use `enqueueAiEdit` |
| `src/App.tsx` | Adapt JS bridge, render global progress bar, add Native progress sync |

### Android Native — Modify

| File | Responsibility |
|------|---------------|
| `ImageViewerActivity.kt` | Remove Toast, add progress bar UI + JS bridge methods |
| `activity_image_viewer.xml` (portrait) | Add progress bar layout above bottom_bar |
| `activity_image_viewer.xml` (landscape) | Add progress bar layout above bottom_bar |
| `drawable/progress_bar_bg.xml` | **NEW** — Progress bar background drawable |
| `drawable/progress_bar_fill.xml` | **NEW** — Progress bar fill drawable |

---

## Task 1: Backend — Progress Event Type

**Files:**
- Create: `src-tauri/src/ai_edit/progress.rs`
- Modify: `src-tauri/src/ai_edit/mod.rs`

- [ ] **Step 1: Create `progress.rs` with the event enum**

```rust
// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AiEditProgressEvent {
    Progress {
        current: u32,
        total: u32,
        #[serde(rename = "fileName")]
        #[ts(rename = "fileName")]
        file_name: String,
        #[serde(rename = "failedCount")]
        #[ts(rename = "failedCount")]
        failed_count: u32,
    },
    Completed {
        current: u32,
        total: u32,
        #[serde(rename = "fileName")]
        #[ts(rename = "fileName")]
        file_name: String,
        #[serde(rename = "failedCount")]
        #[ts(rename = "failedCount")]
        failed_count: u32,
    },
    Failed {
        current: u32,
        total: u32,
        #[serde(rename = "fileName")]
        #[ts(rename = "fileName")]
        file_name: String,
        error: String,
        #[serde(rename = "failedCount")]
        #[ts(rename = "failedCount")]
        failed_count: u32,
    },
    Done {
        total: u32,
        #[serde(rename = "failedCount")]
        #[ts(rename = "failedCount")]
        failed_count: u32,
        #[serde(rename = "failedFiles")]
        #[ts(rename = "failedFiles")]
        failed_files: Vec<String>,
    },
}
```

- [ ] **Step 2: Add `pub mod progress;` to `mod.rs`**

In `src-tauri/src/ai_edit/mod.rs`, add after line 5 (`pub mod config;`):

```rust
pub mod progress;
```

- [ ] **Step 3: Generate TypeScript bindings**

Run: `./build.sh gen-types`

Expected: File `src-tauri/bindings/AiEditProgressEvent.ts` is created.

- [ ] **Step 4: Add type re-export in `src/types/index.ts`**

Add after line 29 (`export type { SeedEditConfig }`):

```typescript
export type { AiEditProgressEvent } from '../../src-tauri/bindings/AiEditProgressEvent';
```

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(ai-edit): add AiEditProgressEvent type with ts-rs bindings"
```

---

## Task 2: Backend — Emit Progress Events from worker_loop

**Files:**
- Modify: `src-tauri/src/ai_edit/service.rs`

This is the core backend change. The worker_loop must emit progress events via `app_handle.emit()`.

- [ ] **Step 1: Add imports and modify `AiEditService` to track queue state**

At the top of `service.rs`, add `use tauri::Emitter;` after the existing `use tauri::{AppHandle, Manager};`.

Add `AtomicU32` import:

```rust
use std::sync::atomic::{AtomicU32, Ordering};
```

Add fields to `AiEditService`:

```rust
pub struct AiEditService {
    manual_sender: mpsc::Sender<AiEditTask>,
    auto_sender: mpsc::Sender<AiEditTask>,
    queue_depth: Arc<AtomicU32>,
}
```

Update `AiEditService::new` — initialize `queue_depth: Arc::new(AtomicU32::new(0))` and pass a clone to the worker_loop.

- [ ] **Step 2: Add `enqueue_manual`, `queue_len`, `cancel` methods to `AiEditService`**

```rust
/// Manual batch enqueue (non-blocking, no result callback).
/// Used by `enqueue_ai_edit` command for batch operations.
pub async fn enqueue_manual(&self, file_path: PathBuf, override_prompt: Option<String>) -> Result<(), AppError> {
    self.queue_depth.fetch_add(1, Ordering::Relaxed);
    self.manual_sender
        .send(AiEditTask {
            file_path,
            is_auto_trigger: false,
            override_prompt,
            result_tx: None,
        })
        .await
        .map_err(|_| AppError::AiEditError("AI edit service shut down".to_string()))
}

pub fn queue_len(&self) -> u32 {
    self.queue_depth.load(Ordering::Relaxed)
}
```

Note: `cancel` will be a no-op for now (complex to implement with mpsc channels). We can add it later if needed.

- [ ] **Step 3: Modify worker_loop to emit progress events**

The worker_loop needs to:
1. Track `completed_count: u32`, `failed_count: u32`, and `failed_files: Vec<String>` across iterations
2. Before processing each task, compute `current = completed_count + failed_count + 1` and `total = current + queue_depth`
3. Emit `Progress` before processing, `Completed`/`Failed` after
4. When both channels are empty (about to block), emit `Done` if any tasks were processed

Refactor the loop to use a helper struct:

```rust
struct WorkerState {
    completed_count: u32,
    failed_count: u32,
    failed_files: Vec<String>,
}

impl WorkerState {
    fn current_index(&self) -> u32 {
        self.completed_count + self.failed_count
    }

    fn reset(&mut self) {
        self.completed_count = 0;
        self.failed_count = 0;
        self.failed_files.clear();
    }
}
```

In the loop, after taking a task from the channel:

```rust
let current = state.current_index() + 1;
let total = current + queue_depth.load(Ordering::Relaxed);
let file_name = task.file_path.file_name()
    .and_then(|n| n.to_str())
    .unwrap_or("unknown")
    .to_string();

let _ = app_handle.emit("ai-edit-progress", &AiEditProgressEvent::Progress {
    current,
    total,
    file_name: file_name.clone(),
    failed_count: state.failed_count,
});
```

After `process_task`:

```rust
match result {
    Ok(ref output_path) => {
        // ... existing indexing logic ...
        state.completed_count += 1;
        let _ = app_handle.emit("ai-edit-progress", &AiEditProgressEvent::Completed {
            current: state.current_index(),
            total: state.current_index() + queue_depth.load(Ordering::Relaxed),
            file_name: file_name.clone(),
            failed_count: state.failed_count,
        });
    }
    Err(ref e) => {
        // ... existing logging ...
        state.failed_count += 1;
        state.failed_files.push(file_name.clone());
        let _ = app_handle.emit("ai-edit-progress", &AiEditProgressEvent::Failed {
            current: state.current_index(),
            total: state.current_index() + queue_depth.load(Ordering::Relaxed),
            file_name: file_name.clone(),
            error: e.to_string(),
            failed_count: state.failed_count,
        });
    }
}
```

Decrement `queue_depth` after each task:

```rust
queue_depth.fetch_sub(1, Ordering::Relaxed);
```

When the loop exits (both channels closed), emit `Done`:

```rust
if state.current_index() > 0 {
    let _ = app_handle.emit("ai-edit-progress", &AiEditProgressEvent::Done {
        total: state.current_index(),
        failed_count: state.failed_count,
        failed_files: state.failed_files,
    });
}
```

- [ ] **Step 4: Update `on_file_uploaded` to increment queue_depth**

In `on_file_uploaded`, before `try_send`, add:

```rust
self.queue_depth.fetch_add(1, Ordering::Relaxed);
```

Note: If `try_send` fails (queue full), decrement back:

```rust
if let Err(e) = self.auto_sender.try_send(...) {
    self.queue_depth.fetch_sub(1, Ordering::Relaxed);
    warn!("AI edit queue full, dropping task: {}", e);
}
```

Similarly for `edit_single` — increment before send, but since it awaits and returns an error on send failure, decrement in the error path.

- [ ] **Step 5: Build to verify**

Run: `./build.sh windows android`

Expected: Clean build, no errors.

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat(ai-edit): emit progress events from worker_loop"
```

---

## Task 3: Backend — New Tauri Commands

**Files:**
- Modify: `src-tauri/src/commands/ai_edit.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add `enqueue_ai_edit` and `cancel_ai_edit` commands to `commands/ai_edit.rs`**

Append to the file:

```rust
#[command]
pub async fn enqueue_ai_edit(
    ai_edit: State<'_, AiEditService>,
    file_paths: Vec<String>,
    prompt: Option<String>,
) -> Result<(), AppError> {
    for path in &file_paths {
        ai_edit.enqueue_manual(PathBuf::from(path), prompt.clone()).await?;
    }
    Ok(())
}
```

- [ ] **Step 2: Re-export from `commands/mod.rs`**

Change line 55 from:
```rust
pub use ai_edit::trigger_ai_edit;
```
to:
```rust
pub use ai_edit::{enqueue_ai_edit, trigger_ai_edit};
```

- [ ] **Step 3: Register in `lib.rs` invoke_handler**

Add imports at line 55 (after `trigger_ai_edit,`):

```rust
    enqueue_ai_edit,
```

Add in the `invoke_handler![]` after `trigger_ai_edit,` (line 223):

```rust
            enqueue_ai_edit,
```

- [ ] **Step 4: Build to verify**

Run: `./build.sh windows android`

Expected: Clean build.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(ai-edit): add enqueue_ai_edit command"
```

---

## Task 4: Frontend — Progress State Store

**Files:**
- Create: `src/hooks/useAiEditProgress.ts`

- [ ] **Step 1: Create the zustand-based progress store and hook**

```typescript
/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { create } from 'zustand';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import type { AiEditProgressEvent } from '../types';

interface AiEditProgressState {
  isEditing: boolean;
  isDone: boolean;
  current: number;
  total: number;
  currentFileName: string;
  failedCount: number;
  failedFiles: string[];
}

const initialState: AiEditProgressState = {
  isEditing: false,
  isDone: false,
  current: 0,
  total: 0,
  currentFileName: '',
  failedCount: 0,
  failedFiles: [],
};

const useAiEditProgressStore = create<AiEditProgressState>(() => ({ ...initialState }));

let listenerCleanup: (() => void) | null = null;
let listenerRefCount = 0;

function handleEvent(event: AiEditProgressEvent) {
  switch (event.type) {
    case 'Progress':
      useAiEditProgressStore.setState({
        isEditing: true,
        isDone: false,
        current: event.current,
        total: event.total,
        currentFileName: event.fileName,
        failedCount: event.failedCount,
      });
      syncToNativeLayer(event.current, event.total, event.failedCount);
      break;
    case 'Completed':
      useAiEditProgressStore.setState({
        failedCount: event.failedCount,
      });
      break;
    case 'Failed':
      useAiEditProgressStore.setState({
        failedCount: event.failedCount,
      });
      break;
    case 'Done': {
      const allFailed = event.failedCount === event.total;
      useAiEditProgressStore.setState({
        isEditing: false,
        isDone: event.failedCount > 0,
        current: event.total,
        failedCount: event.failedCount,
        failedFiles: event.failedFiles,
      });
      if (event.failedCount === 0) {
        setTimeout(() => {
          useAiEditProgressStore.setState({ ...initialState });
        }, 500);
      }
      notifyNativeDone(event.failedCount === 0, event.failedCount, event.failedFiles);
      break;
    }
  }
}

async function ensureListener() {
  if (listenerCleanup) return;
  const unlisten = await listen<AiEditProgressEvent>('ai-edit-progress', (e) => {
    handleEvent(e.payload);
  });
  listenerCleanup = unlisten;
}

function cleanupListener() {
  if (listenerCleanup) {
    listenerCleanup();
    listenerCleanup = null;
  }
}

/** Sync progress to Android Native ImageViewerActivity */
function syncToNativeLayer(current: number, total: number, failedCount: number) {
  window.ImageViewerAndroid?.updateAiEditProgress?.(current, total, failedCount);
}

function notifyNativeDone(success: boolean, failedCount: number, failedFiles: string[]) {
  const message = success
    ? null
    : `修图完成，${failedCount}张失败：${failedFiles.join('、')}`;
  window.ImageViewerAndroid?.onAiEditComplete?.(success, message);
}

export function useAiEditProgress(): AiEditProgressState {
  return useAiEditProgressStore();
}

export async function enqueueAiEdit(files: string[], prompt: string, shouldSave: boolean): Promise<void> {
  await invoke('enqueue_ai_edit', {
    filePaths: files,
    prompt: prompt || null,
  });
}

export function dismissDone() {
  useAiEditProgressStore.setState({ ...initialState });
}

export function useAiEditProgressListener() {
  useEffect(() => {
    listenerRefCount++;
    ensureListener();

    return () => {
      listenerRefCount--;
      if (listenerRefCount <= 0) {
        listenerRefCount = 0;
        cleanupListener();
      }
    };
  }, []);
}

// Re-import useEffect for the hook above
import { useEffect } from 'react';
```

Wait — the import should be at the top. Let me fix:

The final file should have `useEffect` imported at the top alongside other React imports. Remove the bottom re-import.

- [ ] **Step 2: Verify TypeScript compiles**

Run: `npx tsc --noEmit` in the project root (or just verify via build)

- [ ] **Step 3: Commit**

```bash
git add -A && git commit -m "feat(ai-edit): add useAiEditProgress store with backend event listener"
```

---

## Task 5: Frontend — AiEditProgressBar Component

**Files:**
- Create: `src/components/AiEditProgressBar.tsx`

- [ ] **Step 1: Create the unified progress bar component**

```typescript
/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useAiEditProgress } from '../hooks/useAiEditProgress';
import { X } from 'lucide-react';

interface AiEditProgressBarProps {
  position: 'absolute' | 'fixed';
}

export function AiEditProgressBar({ position }: AiEditProgressBarProps) {
  const { isEditing, isDone, current, total, failedCount, failedFiles } = useAiEditProgress();

  if (!isEditing && !isDone) return null;

  const hasFailures = failedCount > 0;
  const progressPercent = total > 0 ? (current / total) * 100 : 0;

  const positionClass = position === 'fixed'
    ? 'fixed bottom-4 left-4 right-4 z-50'
    : 'absolute left-4 right-4 z-10';
  const bottomOffset = position === 'absolute' ? 'bottom-[76px]' : '';
  const positionStyle = position === 'absolute' ? { bottom: '76px' } : undefined;

  return (
    <div
      className={`
        ${position === 'fixed' ? 'fixed bottom-4 left-4 right-4 z-50' : 'absolute left-4 right-4 z-10'}
        transition-all duration-300 ease-in-out
      `}
      style={positionStyle}
    >
      <div
        className={`
          rounded-xl backdrop-blur-sm px-4 py-3 flex items-center gap-3
          transition-colors duration-300
          ${isDone && hasFailures
            ? 'bg-red-500/80'
            : 'bg-black/70'
          }
        `}
      >
        {/* Progress bar */}
        {!isDone && (
          <div className="flex-1">
            <div className="h-1.5 bg-white/20 rounded-full overflow-hidden">
              <div
                className="h-full bg-gradient-to-r from-blue-500 to-blue-400 rounded-full transition-all duration-500 ease-out relative overflow-hidden shimmer"
                style={{ width: `${progressPercent}%` }}
              />
            </div>
          </div>
        )}

        {/* Text */}
        <span className="text-white text-sm font-medium whitespace-nowrap">
          {isDone
            ? `修图完成，${failedCount}张失败`
            : hasFailures
              ? `第${current}张/共${total}张 (失败${failedCount}张)`
              : `第${current}张/共${total}张`
          }
        </span>

        {/* Close/Cancel button */}
        <button
          onClick={() => {
            if (isDone) {
              const { dismissDone } = require('../hooks/useAiEditProgress');
              dismissDone();
            }
          }}
          className="p-1 text-white/60 hover:text-white transition-colors rounded-full hover:bg-white/10"
        >
          <X className="w-4 h-4" />
        </button>
      </div>

      {/* Shimmer animation style */}
      <style>{`
        @keyframes shimmer {
          0% { background-position: -200% 0; }
          100% { background-position: 200% 0; }
        }
        .shimmer {
          background-size: 200% 100%;
          animation: shimmer 2s linear infinite;
        }
      `}</style>
    </div>
  );
}
```

Note: Using `require` for `dismissDone` to avoid circular import issues. Alternatively, import at top.

- [ ] **Step 2: Commit**

```bash
git add -A && git commit -m "feat(ai-edit): add AiEditProgressBar component"
```

---

## Task 6: Frontend — Integrate PreviewWindow

**Files:**
- Modify: `src/components/PreviewWindow.tsx`

- [ ] **Step 1: Remove `aiEditing` state and related handlers, use global progress**

In `PreviewWindowContent`, remove:
- Line 43: `const [aiEditing, setAiEditing] = useState(false);`
- The `handleAiEdit` callback (lines 181-184) — simplify to just open prompt dialog without `aiEditing` check
- The `handlePromptConfirm` callback (lines 186-209) — replace with `enqueueAiEdit` call

Replace `handlePromptConfirm`:

```typescript
const handlePromptConfirm = useCallback(async (prompt: string, shouldSave: boolean) => {
  if (!imagePath) return;
  setShowPromptDialog(false);

  if (shouldSave && prompt !== defaultPrompt) {
    updateDraft(d => ({
      ...d,
      aiEdit: { ...d.aiEdit, prompt },
    }));
  }

  await enqueueAiEdit([imagePath], prompt, shouldSave);
}, [imagePath, defaultPrompt, updateDraft]);
```

Simplify `handleAiEdit`:

```typescript
const handleAiEdit = useCallback(() => {
  if (!imagePath) return;
  setShowPromptDialog(true);
}, [imagePath]);
```

- [ ] **Step 2: Replace AI edit button disabled state with progress bar**

Update the AI edit button (around line 443-460): remove the `disabled={aiEditing}` and the conditional styling based on `aiEditing`. Keep the button always enabled — clicking during editing adds to queue.

Replace:
```tsx
disabled={aiEditing}
className={`
  p-2 rounded-lg transition-colors
  ${aiEditing
    ? 'text-blue-300 bg-blue-500/20 cursor-wait'
    : 'text-gray-300 hover:text-white hover:bg-white/10'
  }
`}
title={aiEditing ? 'AI修图中...' : 'AI修图'}
```

With:
```tsx
className="p-2 rounded-lg transition-colors text-gray-300 hover:text-white hover:bg-white/10"
title="AI修图"
```

- [ ] **Step 3: Add AiEditProgressBar and listener to PreviewWindow**

Import at top:
```typescript
import { AiEditProgressBar } from './AiEditProgressBar';
import { useAiEditProgressListener, enqueueAiEdit } from '../hooks/useAiEditProgress';
```

In the component, add:
```typescript
useAiEditProgressListener();
```

Add the progress bar in the JSX, before the toolbar div (the `absolute bottom-0` toolbar):

```tsx
<AiEditProgressBar position="absolute" />
```

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "feat(ai-edit): integrate progress bar into PreviewWindow"
```

---

## Task 7: Frontend — Integrate Gallery Batch Edit

**Files:**
- Modify: `src/hooks/useGallerySelection.ts`
- Modify: `src/App.tsx`

- [ ] **Step 1: Update `useGallerySelection.ts` — replace toast + fire-and-forget with `enqueueAiEdit`**

Add import at top:
```typescript
import { enqueueAiEdit } from './useAiEditProgress';
```

Remove the `toast` import (line 9) — check if `toast` is still used elsewhere in this file (yes, lines 178 and 184 for delete failures). Keep the import.

Replace `handleAiEditPromptConfirm` (lines 245-274):

```typescript
const handleAiEditPromptConfirm = useCallback(async (prompt: string, shouldSave: boolean) => {
  setShowAiEditPrompt(false);

  if (shouldSave) {
    const draft = useConfigStore.getState().draft;
    if (draft && prompt !== draft.aiEdit.prompt) {
      useConfigStore.getState().updateDraft(d => ({
        ...d,
        aiEdit: { ...d.aiEdit, prompt },
      }));
    }
  }

  const uris = [...selectedIds]
    .map((mediaId) => getUriForId(mediaId))
    .filter((uri): uri is string => uri !== undefined);

  if (uris.length === 0) {
    return;
  }

  const filePaths = uris
    .map((uri) => window.ImageViewerAndroid?.resolveFilePath?.(uri) ?? uri);

  await enqueueAiEdit(filePaths, prompt, shouldSave);
}, [selectedIds, getUriForId]);
```

Key changes:
- Removed `toast.success(...)`
- Replaced `for` loop of `void invoke(...)` with single `await enqueueAiEdit(filePaths, ...)`

- [ ] **Step 2: Update `App.tsx` — add progress bar + adapt JS bridge**

Import:
```typescript
import { AiEditProgressBar } from './components/AiEditProgressBar';
import { useAiEditProgressListener, enqueueAiEdit } from './hooks/useAiEditProgress';
```

Add in the `App` function (after `useAppBootstrap`):
```typescript
useAiEditProgressListener();
```

Update `__tauriTriggerAiEditWithPrompt` to use `enqueueAiEdit`:
```typescript
w.__tauriTriggerAiEditWithPrompt = async (filePath: string, prompt: string, shouldSave: boolean) => {
  const draft = useConfigStore.getState().draft;
  if (shouldSave && draft && prompt !== draft.aiEdit.prompt) {
    updateDraft(d => ({
      ...d,
      aiEdit: { ...d.aiEdit, prompt },
    }));
  }

  await enqueueAiEdit([filePath], prompt, shouldSave);
};
```

Key change: No more `try/catch` with `onAiEditComplete` — that's now handled by the progress event listener.

Add the progress bar in the JSX. For the gallery tab, render inside the gallery container:

After `<GalleryCard />` (line 147), add:
```tsx
<AiEditProgressBar position="fixed" />
```

- [ ] **Step 3: Commit**

```bash
git add -A && git commit -m "feat(ai-edit): integrate progress bar into Gallery and App"
```

---

## Task 8: Android Native — Progress Bar UI

**Files:**
- Create: `src-tauri/gen/android/app/src/main/res/drawable/progress_bar_bg.xml`
- Create: `src-tauri/gen/android/app/src/main/res/drawable/progress_bar_fill.xml`
- Modify: `src-tauri/gen/android/app/src/main/res/layout/activity_image_viewer.xml`
- Modify: `src-tauri/gen/android/app/src/main/res/layout-land/activity_image_viewer.xml`
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/ImageViewerActivity.kt`

- [ ] **Step 1: Create drawable resources**

`drawable/progress_bar_bg.xml`:
```xml
<?xml version="1.0" encoding="utf-8"?>
<shape xmlns:android="http://schemas.android.com/apk/res/android"
    android:shape="rectangle">
    <solid android:color="#00000000" />
    <corners android:radius="3dp" />
</shape>
```

`drawable/progress_bar_fill.xml`:
```xml
<?xml version="1.0" encoding="utf-8"?>
<shape xmlns:android="http://schemas.android.com/apk/res/android"
    android:shape="rectangle">
    <solid android:color="#3B82F6" />
    <corners android:radius="3dp" />
</shape>
```

- [ ] **Step 2: Add progress bar layout to portrait XML**

In `activity_image_viewer.xml`, add between the `ViewPager2` and the `bottom_bar` LinearLayout:

```xml
<!-- AI Edit progress bar -->
<LinearLayout
    android:id="@+id/ai_edit_progress_container"
    android:layout_width="match_parent"
    android:layout_height="wrap_content"
    android:layout_gravity="bottom"
    android:layout_marginStart="16dp"
    android:layout_marginEnd="16dp"
    android:layout_marginBottom="76dp"
    android:background="@drawable/progress_bar_bg"
    android:orientation="horizontal"
    android:gravity="center_vertical"
    android:paddingHorizontal="16dp"
    android:paddingVertical="12dp"
    android:visibility="gone">

    <ProgressBar
        android:id="@+id/ai_edit_progress_bar"
        style="@android:style/Widget.ProgressBar.Horizontal"
        android:layout_width="0dp"
        android:layout_height="6dp"
        android:layout_weight="1"
        android:max="100"
        android:progress="0"
        android:progressDrawable="@drawable/progress_bar_fill" />

    <TextView
        android:id="@+id/ai_edit_progress_text"
        android:layout_width="wrap_content"
        android:layout_height="wrap_content"
        android:textColor="#E5E7EB"
        android:textSize="13sp"
        android:layout_marginStart="12dp" />
</LinearLayout>
```

- [ ] **Step 3: Add same layout to landscape XML**

Same structure in `layout-land/activity_image_viewer.xml`, placed between `ViewPager2` and `bottom_bar`.

- [ ] **Step 4: Update `ImageViewerActivity.kt`**

Add new fields:
```kotlin
private lateinit var aiEditProgressContainer: LinearLayout
private lateinit var aiEditProgressBar: ProgressBar
private lateinit var aiEditProgressText: TextView
```

In `onCreate` / `setupButtons` area, initialize views:
```kotlin
aiEditProgressContainer = findViewById(R.id.ai_edit_progress_container)
aiEditProgressBar = findViewById(R.id.ai_edit_progress_bar)
aiEditProgressText = findViewById(R.id.ai_edit_progress_text)
```

Add method for JS bridge to update progress:
```kotlin
@JavascriptInterface
fun updateAiEditProgress(current: Int, total: Int, failedCount: Int) {
    runOnUiThread {
        if (isFinishing || isDestroyed) return@runOnUiThread
        aiEditProgressContainer.visibility = View.VISIBLE
        val percent = if (total > 0) (current * 100) / total else 0
        aiEditProgressBar.progress = percent
        val text = if (failedCount > 0) {
            "第${current}张/共${total}张 (失败${failedCount}张)"
        } else {
            "第${current}张/共${total}张"
        }
        aiEditProgressText.text = text
    }
}
```

Update `onAiEditComplete` to show done state and hide progress:
```kotlin
fun onAiEditComplete(success: Boolean, message: String?) {
    runOnUiThread {
        isAiEditing = false
        if (isFinishing || isDestroyed) return@runOnUiThread
        if (success) {
            aiEditProgressContainer.visibility = View.GONE
        } else {
            aiEditProgressText.text = message ?: "修图失败"
            aiEditProgressContainer.postDelayed({
                aiEditProgressContainer.visibility = View.GONE
            }, 3000)
        }
    }
}
```

Remove all `Toast.makeText` calls in:
- `triggerAiEditForCurrentImage` (lines 407, 413)
- `dispatchAiEdit` (lines 595, 615)
- `onAiEditComplete` (line 652)

Update `dispatchAiEdit` — remove the `isAiEditing = true` and `Toast` at line 594-595, since progress will be shown via the bridge.

- [ ] **Step 5: Register the JS bridge in `MainActivity.kt`**

Ensure `updateAiEditProgress` is accessible from WebView. Check how `ImageViewerAndroid` bridge is registered — add the method there if needed.

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat(android): replace AI edit Toast with native progress bar"
```

---

## Task 9: Build Verification

- [ ] **Step 1: Generate TypeScript bindings**

Run: `./build.sh gen-types`

- [ ] **Step 2: Full build**

Run: `./build.sh windows android`

Expected: Both platforms build successfully.

- [ ] **Step 3: Fix any compilation errors**

If errors, fix and rebuild.

- [ ] **Step 4: Commit any fixes**

```bash
git add -A && git commit -m "fix: resolve build errors from AI edit progress bar integration"
```

---

## Task 10: Cleanup — Remove Unused Toast Import

**Files:**
- Modify: `src/hooks/useGallerySelection.ts`

- [ ] **Step 1: Check if `toast` from `sonner` is still used**

If all `toast` calls related to AI edit have been removed and no other `toast` calls remain in the file, remove the import:

```typescript
// Remove this line if no other toast calls exist:
import { toast } from 'sonner';
```

If `toast` is still used for delete error notifications (lines 178, 184), keep the import.

- [ ] **Step 2: Commit**

```bash
git add -A && git commit -m "chore: clean up unused toast import"
```
