# Media Refresh and Virtual Root Design

**Date:** 2026-04-12

**Goal:** Fix Android latest-photo refresh behavior after arbitrary file support landed, and make the Android FTP virtual root aggregate both media and non-media storage roots without changing the Windows preview model.

## Confirmed Problems

1. On Android, newly received RAW files do not refresh the home page latest-photo card in real time, while JPG files do.
2. On Android, the FTP virtual directory only exposes `DCIM/CameraFTP` content and omits non-media files stored under `Download/CameraFTP`.
3. When the virtual root aggregates both storage roots, path collisions must be resolved deterministically.
4. Windows built-in preview uses the Tauri WebView `<img>` path, so expanding its latest-photo source to arbitrary media would risk pointing the preview UI at unsupported RAW files.

## Current Behavior Summary

### Android latest-photo refresh

- Gallery/media additions emit `gallery-items-added` from `MediaStoreBridge.kt` when the file MIME starts with `image/` or `video/`.
- `useLatestPhoto.ts` does not listen to `gallery-items-added`; it only listens to `latest-photo-refresh-requested` and Tauri `file-index-changed`.
- `file-index-changed` is driven by `FileIndexService::is_supported_image()`, which only includes `jpg`, `jpeg`, `heif`, `hif`, and `heic`.
- Result: RAW files are added to Android media flows but do not refresh the homepage latest-photo card.

### Windows latest-photo preview

- Windows latest-photo comes from `get_latest_image`, which reads the filtered file index.
- The preview window renders the selected file through `<img src={convertFileSrc(imagePath)}>`.
- RAW files are currently excluded by the file-index filter, which protects the built-in WebView preview from unsupported formats.

### Android virtual FTP root

- Media uploads are stored under `DCIM/CameraFTP/`.
- Non-media uploads are stored under `Download/CameraFTP/`.
- `AndroidMediaStoreBackend::list()` resolves one directory prefix and queries only that prefix.
- The virtual root therefore only shows the `DCIM/CameraFTP/` side.

## Approved Design

### 1. Platform-specific latest-photo refresh policy

#### Android

- Treat `gallery-items-added` as the authoritative signal for newly received media.
- Refresh latest-photo when that event fires.
- Because the Android event already filters to `image/*` and `video/*`, non-media files will not trigger refresh.

#### Windows

- Do not broaden the latest-photo refresh source.
- Keep using the existing file-index-based refresh path.
- This preserves the current WebView-safe behavior and avoids exposing unsupported RAW files to the built-in preview flow.

### 2. Aggregated Android virtual root

- The Android FTP virtual root becomes a merged view of:
  - `DCIM/CameraFTP/`
  - `Download/CameraFTP/`
- The same rule applies recursively to child directories.
- Operations that resolve a virtual path (`list`, `metadata`, `directory_exists`, `get`, and delete-related lookups) must use the same merged resolution policy so visible items remain accessible.

### 3. Collision policy (approved option A)

- Directory + directory: merge.
- File + file: prefer `DCIM/CameraFTP` and hide the `Download/CameraFTP` item.
- Directory + file: prefer the directory so its subtree remains reachable.
- Log a warning whenever a collision is resolved by precedence.

## Non-Goals

- No large-scale Windows preview refactor.
- No RAW decoding support inside the Windows WebView preview.
- No changes to Android storage locations introduced by the previous arbitrary-file support work.

## Verification Targets

1. Android latest-photo refreshes after JPG, HEIF, RAW, and video additions.
2. Android latest-photo does not refresh after non-media additions.
3. Windows latest-photo behavior remains limited to WebView-supported indexed image types.
4. Android FTP root shows entries from both `DCIM/CameraFTP` and `Download/CameraFTP`.
5. Collisions resolve consistently using the approved precedence rules.
