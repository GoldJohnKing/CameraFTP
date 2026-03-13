# MediaStore Reliability Design (Android 13+)

Date: 2026-03-13

## Goals

- Make MediaStore the single source of truth for gallery listing and external opening.
- Ensure first open always targets the latest uploaded image.
- Remove reliance on MANAGE_EXTERNAL_STORAGE.
- Support swipe browsing in external gallery apps using MediaStore URIs.
- Minimize extra memory copies by streaming FTP data directly into MediaStore via fd.

## Non-Goals

- Support Android 11/12 (minSdk is raised to 33).
- Maintain file-system based indexing for Android.
- Add new user-facing UI or delays.

## Architecture Overview

- MediaStore is authoritative for Android gallery data and external app opening.
- Rust does not write to external storage paths directly.
- Android native code creates a MediaStore entry and returns a file descriptor (fd).
- Rust streams FTP bytes into that fd (zero extra JS or userland copies beyond OS buffering).
- A custom Android storage backend for libunftp coordinates fd acquisition per upload.
- After write completion, Android finalizes the entry (`IS_PENDING=0`) and emits a new
  event `media-store-ready` for front-end refresh.

## Data Flow

1) FTP upload begins (Rust, libunftp).
2) Android storage backend requests MediaStore entry creation:
   - file name
   - MIME type
   - relative path: `DCIM/CameraFTP/` (standardized with trailing slash)
   - optional size hint
3) Android inserts into MediaStore with `IS_PENDING=1`, returns:
   - fd (ParcelFileDescriptor)
   - content URI
4) Rust streams bytes into the fd.
5) Rust notifies Android to finalize the entry.
6) Android sets `IS_PENDING=0`, verifies entry, emits `media-store-ready`.
7) Frontend listens to `media-store-ready` and refreshes gallery from MediaStore.
8) External open uses MediaStore URIs only, with same-directory URIs for swipe browsing.

## Eventing

- New event: `media-store-ready`.
- Payload should include:
  - content URI
  - relative path
  - display name
  - size
  - timestamp
- Frontend uses this as the sole trigger to refresh the gallery list.
- Existing `file-uploaded` stays for stats, but no longer triggers refresh.

## MediaStore Querying

- Queries filter by `RELATIVE_PATH = "DCIM/CameraFTP/"` and `MEDIA_TYPE = IMAGE`.
- Only the root directory is included (no subdirectories).
- Ordering by `DATE_MODIFIED DESC`, ties broken by `DATE_ADDED` and size.
- Note: This changes ordering semantics from EXIF-based sort. This is intentional for
  consistency with MediaStore authority; if needed, a future user setting can reintroduce
  EXIF sorting from MediaStore metadata.
- The gallery list and external open both use this MediaStore result set.
- Empty result: show the existing empty-state UI, not an error.

## Open External App

- Only MediaStore content URIs are used.
- Build ClipData with a capped window of URIs (target plus 25 before/after)
  to stay below Binder size limits (51 URIs x ~200B ≈ 10KB).
- No FileProvider fallback paths.
- If no activity can handle the intent, show a minimal error toast.

## Permissions and SDK

- minSdk: 33 (Android 13+).
- Required permission: `READ_MEDIA_IMAGES`.
- Remove any reliance on `MANAGE_EXTERNAL_STORAGE` for Android.

## Error Handling

- If MediaStore insert fails: abort upload, report error to Rust and frontend.
- If fd write fails: delete the pending MediaStore row, report error.
- If finalize fails: delete the row and report error; do not refresh gallery.
- No fallback to file-system indexing to avoid split-brain state.
- If permission is denied: show existing permission onboarding UI and block gallery.
- If storage is full (ENOSPC): abort upload, delete pending row, report error.

## Reliability Guarantees

- `media-store-ready` is emitted only after `IS_PENDING=0` succeeds.
- First open uses MediaStore URIs generated from the authoritative list.
- Swipe browsing is consistent with the gallery list.
- Event order does not affect list order because refresh performs a full MediaStore query.

## Android Bridge Changes

- New Android bridge for MediaStore lifecycle:
  - `createMediaStoreEntry(name, mime, relativePath, sizeHint) -> { fd, uri }`
  - `finalizeMediaStoreEntry(uri)`
  - `abortMediaStoreEntry(uri)`
- Existing gallery list APIs are updated to query MediaStore.
- New media list API returns MediaStore IDs and URIs to avoid path dependency.

## FD Transfer Mechanism

- Android uses `ParcelFileDescriptor.detachFd()` to obtain a raw integer fd.
- The fd is returned through Tauri IPC as an integer handle.
- Rust converts the integer to `OwnedFd` and writes the upload stream using safe ownership APIs.
- Android retains no further handle after `detachFd()` to avoid double-close.
- Rust is responsible for closing the fd after upload completion or error.
- If the process terminates, the OS closes the fd; stale pending rows are cleaned on startup.
- Tauri IPC must return the fd synchronously; Rust assumes ownership immediately.
- This relies on Tauri Android running Rust and Android in a single process; if not,
  use a local pipe or socket bridge and accept an extra copy.

## Rust Changes

- FTP write path uses fd-based writer instead of external file path.
- Android-specific IPC/JNI path to request fd and finalize entry.
- Custom libunftp storage backend for Android:
  - Maps virtual FTP paths to MediaStore display names
  - Requests fd via IPC for each upload
  - Writes to fd stream
  - Enforces an upload concurrency limit (default 4) to avoid fd exhaustion
  - On any write error, calls `abortMediaStoreEntry(uri)`
  - When limit reached, queue uploads until a slot frees

## Frontend Changes

- Refresh gallery on `media-store-ready` only.
- Use MediaStore list provider for images.
- Open external app with MediaStore URIs only.
- Remove FileProvider-only code paths for open and share.
- Initial gallery load performs a MediaStore query on mount.

## Gallery Operations (Android)

- Share: build `Intent.ACTION_SEND(_MULTIPLE)` with MediaStore URIs.
- Delete: delete by MediaStore URI (not by file path).

## Migration Notes

- Existing files in `DCIM/CameraFTP` will appear if already indexed.
- If device has unindexed files, they can be indexed via a one-time MediaStore scan
  or a user-triggered refresh that requests Android to rescan that directory.
- On app start and before creating new entries, clean stale `IS_PENDING=1` rows older than 24h.
- On create, if a row with the same display name exists in the same relative path,
  update it in place (preserve URI) and write new content to its fd.

## Validation Plan

- Upload new image and open immediately: must show correct image.
- Upload same filename with different content: first open must show latest.
- Burst upload 10+ files: gallery order and swipe order must match.
- Verify no MANAGE_EXTERNAL_STORAGE is requested and READ_MEDIA_IMAGES works.
- Verify share and delete operations use MediaStore URIs only.

## Operational Details

- MIME detection order: FTP type if provided, else file extension, else `application/octet-stream`.
- Size hint is optional; actual size is validated on finalize.
- Retry policy: `create` and `finalize` each retry up to 3 times with backoff
  (100ms, 200ms, 400ms).

## Update-in-Place Rules

- For overwrite semantics, set `IS_PENDING=1` on the existing URI before writing.
- When writing completes, set `IS_PENDING=0` and emit `media-store-ready`.

## Thumbnail Strategy

- Generation: `ContentResolver.loadThumbnail()` with MediaStore URI.
- Cache: `cacheDir/thumbnails` keyed by `hash(uri + dateModified)`.
- Eviction: LRU policy, 100MB cap.
- On `media-store-ready`, evict the cached file entry for that URI if present.
