# Known Deferred Issues

Tracking notes for review findings deliberately deferred (decided at 2026-08 review session).
Ledger refreshed 2026-09 (fix-forward): entries are removed when their fix merges, and new known
limitations are appended in the same merge (see AGENTS.md → Deferred Issues Ledger).

## 1. Android `GalleryBridge.deleteImages` blocks the WebView JavaBridge thread up to 30s

`MainActivity.requestDeleteConfirmation` blocks on a `CountDownLatch` while the system delete
confirmation dialog is open. Because the JS bridge thread is single-threaded, every other
`@JavascriptInterface` call (gallery scrolling, EXIF, thumbnails) queues behind it for the duration
of the dialog. The confirmation path only fires for files the app does not own (SecurityException
fallback), so impact is rare.

Deferred because the proper fix (async result delivery via `emitWindowEvent` + TS-side update) is a
destructive-action flow that must be device-tested; a blind conversion risks confirmed-but-reported-
failed deletes or never-resolving promises, and a naive timeout reduction trades a rare freeze for
spurious delete failures on slow users. Handle in a session with an Android device available.

## 2. Windows preview cache: gallery-initiated deletes bypass invalidation

`FileIndexService::remove_file` invalidates the Windows `ImagePreviewCache` entry for the removed
path (covers FTP-session deletes on all backends and filesystem-watcher deletes). The Kotlin
`GalleryBridge.deleteImages` path, however, deletes via `ContentResolver` directly and does not
notify Rust — but note it only applies to Android, where `image_preview` (a Windows-only module) is
not compiled, so no stale-cache window exists today. If `image_preview` is ever enabled for other
platforms, gallery-delete paths must call into the cache invalidation as well.

## 3. `RaNnConfig` FFI field order has no compile-time guard

`src-tauri/src/color_grading/ffi.rs` `RaNnConfig` is `#[repr(C)]` and its comment declares
"Field order MUST match the C struct exactly", but nothing enforces it — no `size_of`/offset
assertion exists. A naive `size_of::<RaNnConfig>()` assert was considered and rejected: the struct
crosses Windows x64 (LLP64) and Android arm64 (LP64), so expected byte sizes/offsets can legitimately
differ per target, and a wrong constant would break one platform's build. If raw-alchemy ever gains
a versioned ABI, add per-target expected sizes (cfg-gated consts) or generate the assert from the
C headers; until then, treat any edit to `RaNnConfig` fields as a cross-platform ABI change requiring
both `./build.sh windows android` and on-device NN smoke tests.

## 4. File watcher event channel (capacity 1000) drops events under large camera bursts

The file watcher event channel in `file_index/watcher.rs` has capacity 1000. A camera burst that
produces more than 1000 events before the consumer drains the channel overflows it; the excess
events are dropped with a `warn` log and the watcher itself keeps running (pinned by the
`channel_full_drops_event_with_warning_without_panic` test in `watcher.rs`). Until a self-heal
mechanism exists, files behind dropped events stay out of the index until the next full scan.
A needs-rescan flag plus a merged rescan on channel drain is being added by a parallel change in
this same branch; the final state after this branch merges is that overflow drops self-heal via
the merged rescan, leaving no index gap.
