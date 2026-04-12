# Media Refresh and Virtual Root Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Android latest-photo refresh on any newly added media item, keep Windows limited to WebView-safe previewable image types, and aggregate `DCIM/CameraFTP` plus `Download/CameraFTP` into one Android FTP virtual root with deterministic collision handling.

**Architecture:** Keep refresh policy platform-specific instead of forcing a single cross-platform event contract. Android latest-photo will subscribe to `gallery-items-added`, while Windows will stay on the existing `file-index-changed` path. On the Android MediaStore backend, add a virtual-path aggregation layer that resolves one FTP path against both storage roots, merges listings, and applies the approved precedence rules (`DCIM` wins file collisions, directories win file-vs-directory collisions).

**Tech Stack:** React + Vitest, Rust `libunftp` Android MediaStore backend, Kotlin MediaStore bridge, `./build.sh windows android` verification.

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `src/hooks/useLatestPhoto.ts` | Modify | Add Android-only media-add refresh subscription without changing Windows behavior |
| `src/hooks/__tests__/useLatestPhoto.test.tsx` | Modify | Lock the new Android refresh behavior and confirm singleton listener cleanup |
| `src-tauri/src/ftp/android_mediastore/backend.rs` | Modify | Add virtual path aggregation, collision resolution, and shared lookup helpers |
| `src-tauri/src/ftp/android_mediastore/tests.rs` | Modify | Verify aggregated listing and precedence rules against the mock bridge |

---

### Task 1: Add Android-only latest-photo refresh on media additions

**Files:**
- Modify: `src/hooks/useLatestPhoto.ts`
- Test: `src/hooks/__tests__/useLatestPhoto.test.tsx`

- [ ] **Step 1: Extend the existing hook test with an Android media-add scenario**

In `src/hooks/__tests__/useLatestPhoto.test.tsx`, mock `isGalleryV2Available()` so each test can choose Android or non-Android behavior.

Add this mock near the existing `latest-photo` service mock:

```ts
const { listenMock, fetchLatestPhotoFileMock, isGalleryV2AvailableMock } = vi.hoisted(() => ({
  listenMock: vi.fn(),
  fetchLatestPhotoFileMock: vi.fn(),
  isGalleryV2AvailableMock: vi.fn(),
}));

vi.mock('../../services/latest-photo', () => ({
  fetchLatestPhotoFile: fetchLatestPhotoFileMock,
}));

vi.mock('../../services/gallery-media-v2', () => ({
  isGalleryV2Available: isGalleryV2AvailableMock,
}));
```

In `beforeEach`, set the default to desktop behavior:

```ts
isGalleryV2AvailableMock.mockReset();
isGalleryV2AvailableMock.mockReturnValue(false);
```

Add these two tests:

```ts
it('refreshes latest photo on gallery-items-added only when Gallery V2 is available', async () => {
  isGalleryV2AvailableMock.mockReturnValue(true);

  await act(async () => {
    root.render(<LatestPhotoHarness />);
    await flush();
  });

  fetchLatestPhotoFileMock.mockResolvedValueOnce({
    filename: 'latest-raw.dng',
    path: 'content://latest-raw',
  });

  await act(async () => {
    window.dispatchEvent(
      new CustomEvent('gallery-items-added', {
        detail: {
          items: [{ mediaId: '1', uri: 'content://latest-raw', displayName: 'latest-raw.dng' }],
          timestamp: Date.now(),
        },
      }),
    );
    await flush();
  });

  expect(fetchLatestPhotoFileMock).toHaveBeenCalledTimes(2);
  expect(container.querySelector('[data-testid="filename"]')?.textContent).toBe('latest-raw.dng');
});

it('ignores gallery-items-added on desktop/webview preview path', async () => {
  isGalleryV2AvailableMock.mockReturnValue(false);

  await act(async () => {
    root.render(<LatestPhotoHarness />);
    await flush();
  });

  await act(async () => {
    window.dispatchEvent(
      new CustomEvent('gallery-items-added', {
        detail: {
          items: [{ mediaId: '1', uri: 'content://raw', displayName: 'raw.dng' }],
          timestamp: Date.now(),
        },
      }),
    );
    await flush();
  });

  expect(fetchLatestPhotoFileMock).toHaveBeenCalledTimes(1);
});
```

- [ ] **Step 2: Run the hook test to verify the new Android case fails first**

Run: `npm test -- useLatestPhoto.test.tsx`

Expected: the new `gallery-items-added` Android test fails because `useLatestPhoto.ts` does not subscribe to that event yet.

- [ ] **Step 3: Add Android-only `gallery-items-added` subscription in `useLatestPhoto.ts`**

Update `src/hooks/useLatestPhoto.ts` imports:

```ts
import { isGalleryV2Available } from '../services/gallery-media-v2';
```

Inside `initializeStore()`, add a Gallery V2-only listener alongside `LATEST_PHOTO_REFRESH_REQUESTED_EVENT`:

```ts
  const handleGalleryItemsAdded = () => {
    void refreshLatestPhoto();
  };

  const shouldListenForGalleryAdds = isGalleryV2Available();

  if (shouldListenForGalleryAdds) {
    window.addEventListener('gallery-items-added', handleGalleryItemsAdded);
  }
```

Then update `teardownFn` so it removes the listener only when it was registered:

```ts
  teardownFn = () => {
    window.removeEventListener(LATEST_PHOTO_REFRESH_REQUESTED_EVENT, handleRefreshRequest);
    if (shouldListenForGalleryAdds) {
      window.removeEventListener('gallery-items-added', handleGalleryItemsAdded);
    }
    void unlistenPromise.then((unlisten) => unlisten()).catch(() => {});
  };
```

This keeps Android responsive to any media item while preserving the Windows WebView-safe filter path.

- [ ] **Step 4: Re-run the hook test**

Run: `npm test -- useLatestPhoto.test.tsx`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/hooks/useLatestPhoto.ts src/hooks/__tests__/useLatestPhoto.test.tsx
git commit -m "fix(ui): refresh android latest photo on media additions"
```

---

### Task 2: Add aggregated virtual-root listing with collision precedence

**Files:**
- Modify: `src-tauri/src/ftp/android_mediastore/backend.rs`
- Test: `src-tauri/src/ftp/android_mediastore/tests.rs`

- [ ] **Step 1: Add failing Rust tests for merged root behavior**

In `src-tauri/src/ftp/android_mediastore/tests.rs`, add helper constructors near the existing mock-backed tests:

```rust
fn query_result(relative_path: &str, display_name: &str, modified: u64, mime_type: &str) -> QueryResult {
    QueryResult {
        content_uri: format!("content://{relative_path}{display_name}"),
        display_name: display_name.to_string(),
        size: 1,
        date_modified: modified,
        mime_type: mime_type.to_string(),
        relative_path: relative_path.to_string(),
    }
}
```

Add these tests:

```rust
#[tokio::test]
async fn list_root_merges_dcim_and_download_entries() {
    let bridge = Arc::new(MockMediaStoreBridge::new());
    bridge.add_query_files_result(
        "DCIM/CameraFTP/",
        vec![query_result("DCIM/CameraFTP/", "photo.jpg", 20, "image/jpeg")],
    );
    bridge.add_query_files_result(
        "Download/CameraFTP/",
        vec![query_result("Download/CameraFTP/", "notes.txt", 10, "text/plain")],
    );

    let backend = AndroidMediaStoreBackend::new_with_bridge(bridge);
    let items = backend.list(&DefaultUser {}, "/").await.unwrap();
    let names = items.into_iter().map(|item| item.path.to_string_lossy().to_string()).collect::<Vec<_>>();

    assert!(names.contains(&"photo.jpg".to_string()));
    assert!(names.contains(&"notes.txt".to_string()));
}

#[tokio::test]
async fn list_root_prefers_dcim_when_same_file_exists_in_both_roots() {
    let bridge = Arc::new(MockMediaStoreBridge::new());
    bridge.add_query_files_result(
        "DCIM/CameraFTP/",
        vec![query_result("DCIM/CameraFTP/", "dup.jpg", 20, "image/jpeg")],
    );
    bridge.add_query_files_result(
        "Download/CameraFTP/",
        vec![query_result("Download/CameraFTP/", "dup.jpg", 30, "image/jpeg")],
    );

    let backend = AndroidMediaStoreBackend::new_with_bridge(bridge);
    let items = backend.list(&DefaultUser {}, "/").await.unwrap();
    let duplicates = items
        .into_iter()
        .filter(|item| item.path == PathBuf::from("dup.jpg"))
        .count();

    assert_eq!(duplicates, 1);
}

#[tokio::test]
async fn list_root_prefers_directory_over_file_when_names_collide() {
    let bridge = Arc::new(MockMediaStoreBridge::new());
    bridge.add_query_files_result(
        "DCIM/CameraFTP/",
        vec![query_result("DCIM/CameraFTP/folder/", "child.jpg", 20, "image/jpeg")],
    );
    bridge.add_query_files_result(
        "Download/CameraFTP/",
        vec![query_result("Download/CameraFTP/", "folder", 10, "text/plain")],
    );

    let backend = AndroidMediaStoreBackend::new_with_bridge(bridge);
    let items = backend.list(&DefaultUser {}, "/").await.unwrap();
    let folder = items.into_iter().find(|item| item.path == PathBuf::from("folder")).unwrap();

    assert!(folder.metadata.is_dir());
}
```

- [ ] **Step 2: Run the focused Rust test target and confirm the new merged-root tests fail**

Run: `cargo.exe test -p camera_ftp_companion_lib android_mediastore::tests::list_root_ -- --nocapture`

Expected: FAIL because the backend still lists only one prefix.

- [ ] **Step 3: Implement shared root helpers and merged listing in `backend.rs`**

In `src-tauri/src/ftp/android_mediastore/backend.rs`, add these constants near `DOWNLOADS_RELATIVE_PATH` usage if they are not already local to the file:

```rust
const MEDIA_ROOT: &str = "DCIM/CameraFTP/";
const DOWNLOAD_ROOT: &str = "Download/CameraFTP/";
```

Add helper methods on `AndroidMediaStoreBackend`:

```rust
    fn virtual_relative_suffix(&self, path: &Path) -> String {
        let normalized = self.normalize_path(path);
        if normalized.as_os_str().is_empty() {
            String::new()
        } else {
            format!("{}/", normalized.to_string_lossy().trim_end_matches('/'))
        }
    }

    fn resolve_directory_prefixes(&self, path: &Path) -> Vec<String> {
        let suffix = self.virtual_relative_suffix(path);
        vec![format!("{MEDIA_ROOT}{suffix}"), format!("{DOWNLOAD_ROOT}{suffix}")]
    }

    fn resolve_file_candidates(&self, path: &Path) -> Vec<String> {
        let normalized = self.normalize_path(path).to_string_lossy().to_string();
        vec![format!("{MEDIA_ROOT}{normalized}"), format!("{DOWNLOAD_ROOT}{normalized}")]
    }
```

Replace the single-prefix `list()` implementation with a two-prefix query/merge flow:

```rust
        let mut merged = Vec::new();
        for prefix in self.resolve_directory_prefixes(path) {
            let bridge = self.bridge.clone();
            let prefix_clone = prefix.clone();
            let mut results = retry_with_backoff(&self.retry_config, "list", || {
                let bridge = bridge.clone();
                let path = prefix_clone.clone();
                async move { bridge.query_files(&path).await }
            })
            .await
            .map_err(|e| Self::file_not_available(e.to_string()))?;
            merged.append(&mut results);
        }

        Ok(self.build_merged_directory_listing(path, merged))
```

Refactor `build_directory_listing()` into a new merged implementation that keys by child name and source precedence:

```rust
enum VirtualEntryKind {
    Directory { modified: u64 },
    File(Fileinfo<PathBuf, MediaStoreMetadata>),
}
```

Rules inside the merge loop:

- If either side says directory, keep directory.
- If both are files with the same child name, keep the `DCIM/CameraFTP` file.
- If both are directories, keep one directory and use the max modified timestamp.
- Emit `tracing::warn!` on file/file and dir/file collisions.

- [ ] **Step 4: Re-run the Rust merged-root tests**

Run: `cargo.exe test -p camera_ftp_companion_lib android_mediastore::tests::list_root_ -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/ftp/android_mediastore/backend.rs src-tauri/src/ftp/android_mediastore/tests.rs
git commit -m "fix(android): merge dcim and download virtual root listings"
```

---

### Task 3: Route metadata and file lookups through the same merged resolution rules

**Files:**
- Modify: `src-tauri/src/ftp/android_mediastore/backend.rs`
- Test: `src-tauri/src/ftp/android_mediastore/tests.rs`

- [ ] **Step 1: Add failing tests for metadata/file lookup precedence**

In `src-tauri/src/ftp/android_mediastore/tests.rs`, add:

```rust
#[tokio::test]
async fn metadata_falls_back_to_download_root_when_media_root_misses() {
    let bridge = Arc::new(MockMediaStoreBridge::new());
    bridge.add_query_file_error("DCIM/CameraFTP/notes.txt", MediaStoreError::NotFound("missing".into()));
    bridge.add_query_file_result(
        "Download/CameraFTP/notes.txt",
        query_result("Download/CameraFTP/", "notes.txt", 10, "text/plain"),
    );

    let backend = AndroidMediaStoreBackend::new_with_bridge(bridge);
    let metadata = backend.metadata(&DefaultUser {}, "notes.txt").await.unwrap();

    assert!(!metadata.is_dir());
    assert_eq!(metadata.len(), 1);
}

#[tokio::test]
async fn metadata_prefers_directory_shape_when_virtual_name_is_directory() {
    let bridge = Arc::new(MockMediaStoreBridge::new());
    bridge.add_query_file_error("DCIM/CameraFTP/folder", MediaStoreError::NotFound("missing".into()));
    bridge.add_query_file_result(
        "Download/CameraFTP/folder",
        query_result("Download/CameraFTP/", "folder", 10, "text/plain"),
    );
    bridge.add_query_files_result(
        "DCIM/CameraFTP/folder/",
        vec![query_result("DCIM/CameraFTP/folder/", "child.jpg", 20, "image/jpeg")],
    );
    bridge.add_query_files_result("Download/CameraFTP/folder/", vec![]);

    let backend = AndroidMediaStoreBackend::new_with_bridge(bridge);
    let metadata = backend.metadata(&DefaultUser {}, "folder").await.unwrap();

    assert!(metadata.is_dir());
}
```

- [ ] **Step 2: Run the focused metadata tests to verify they fail first**

Run: `cargo.exe test -p camera_ftp_companion_lib android_mediastore::tests::metadata_ -- --nocapture`

Expected: FAIL because `metadata()` and `directory_exists()` still resolve only one root.

- [ ] **Step 3: Implement shared candidate lookup helpers and reuse them in `metadata()`, `directory_exists()`, and file open/delete paths**

In `src-tauri/src/ftp/android_mediastore/backend.rs`, add helpers like:

```rust
    async fn query_first_existing_file(&self, path: &Path) -> Result<QueryResult, MediaStoreError> {
        for candidate in self.resolve_file_candidates(path) {
            match self.bridge.query_file(&candidate).await {
                Ok(result) => return Ok(result),
                Err(MediaStoreError::NotFound(_)) => continue,
                Err(err) => return Err(err),
            }
        }

        Err(MediaStoreError::NotFound(path.display().to_string()))
    }
```

Then update:

- `directory_exists()` to query both directory prefixes and return true if either side has children.
- `metadata()` to:
  1. ask `query_first_existing_file()` for file candidates in precedence order,
  2. if no file is found, call the merged `directory_exists()` and synthesize a directory metadata result when appropriate.
- `get()` and any delete path lookup to resolve against the same candidate order so list/metadata/open all agree.

- [ ] **Step 4: Re-run the focused metadata tests**

Run: `cargo.exe test -p camera_ftp_companion_lib android_mediastore::tests::metadata_ -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/ftp/android_mediastore/backend.rs src-tauri/src/ftp/android_mediastore/tests.rs
git commit -m "fix(android): align virtual-root lookups with merged listing rules"
```

---

### Task 4: Full verification

**Files:**
- Modify: none

- [ ] **Step 1: Run the frontend targeted tests**

Run: `npm test -- useLatestPhoto.test.tsx`

Expected: PASS.

- [ ] **Step 2: Run the Android MediaStore Rust tests**

Run: `cargo.exe test -p camera_ftp_companion_lib android_mediastore::tests -- --nocapture`

Expected: PASS.

- [ ] **Step 3: Run the required full build verification**

Run: `./build.sh windows android`

Expected: both Windows and Android builds succeed.

- [ ] **Step 4: Commit verification-only follow-ups if needed**

If the verification steps required small fixes, commit them with a focused message before merging.

---

## Self-Review

- Spec coverage: Android media refresh, Windows RAW avoidance, merged virtual root, and collision policy are all represented in Tasks 1-4.
- Placeholder scan: no `TODO`/`TBD` placeholders remain.
- Type consistency: the plan reuses the existing `QueryResult`, `MediaStoreMetadata`, and `useLatestPhoto` abstractions instead of inventing parallel types.
