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

## 4. Gallery date-jump still full-loads the library (bounded `loadAll` not wired)

`useGalleryPager.loadAll({ untilMs, marginPages })` (bounded: stops at the target capture day + 2
margin pages, cursor preserved) landed with tests in 2026-09, but the only consumer
`GalleryCard.handleOpenDateJump` still calls the no-arg `loadAll()` when the date picker opens,
because `dateOptions` is derived from loaded `pager.items` and there is no separate date-enumeration
API. Wiring this up is a product decision: (a) keep full-load on picker open (status quo), (b) load
to target only after the user picks a date (picker list then shows partial dates), or (c) add a
month-bucketed enumeration API. Until decided, the memory benefit of bounded loadAll is unrealized
end-to-end (hook capability + tests are ready).

## 5. Banded lens-correction: lensfun block calls drift up to 3.05e-05 px from a full-image call

`lens_correction.cpp` now remaps in 128-row bands (coords table peak 576MB → ~18MB). Our banding and
remap code is byte-exact (A/B memcmp across band heights 8/67/200, see
`Test/cpp/test_lens_correction_banding.cpp`), but lensfun accumulates row coordinates in float32
starting at each block's y origin, so per-band `ApplySubpixelGeometryDistortion` values can differ
from the corresponding rows of a full-image call by ≤3.05e-05 px (informational test 3 in the same
file). Consequence: exported images are no longer bit-identical to pre-banding output; the deviation
is ~3 orders of magnitude below 16-bit quantization and visually nil. Not fixable from the caller
side without reverting to full-table generation.

## 6. RAW pipeline peak-memory floor: demosaic window (~624MB @24MP) needs streaming redesign

After early LibRaw `recycle()` (decode window ≈432MB @24MP, byte-identical output) and banded lens
correction (LC stage ≈594MB → src+dst full buffers are inherent to arbitrary-row remap), the
remaining end-to-end peak sits in the RCD/Markesteijn demosaic callback window where LibRaw
`image[]` + `cfa` float + `rgb` float must coexist by kernel contract (measured 1,175MB for a 45.7MP
NEF; ≈624MB @24MP). Going lower requires streaming/in-place demosaic changes or releasing
`rawdata.raw_alloc` mid-demosaic via LibRaw private-ownership hacks (rejected: behavior risk).
Revisit only if on-device RSS measurements still trip MIUI's kill threshold during background
grading batches.

## 7. Processing FGS: missing edge-semantics unit tests; onTimeout leaves Rust flag stale

`ProcessingForegroundService` / `AndroidServiceStateCoordinator.syncNativeProcessingState` /
`processing_activity::notify_one` have no unit tests for their edge semantics (false→true restart
when instance==null, stale-start defense, interleaved pipeline reports, the transient
true→false→true flap documented in `processing_activity.rs`). Manifest assertions are covered.
Also accepted-as-is: on 6h `mediaProcessing` timeout the Kotlin flag clears but Rust
`SYNCED_COMBINED_ACTIVE` stays true, so a later real activation edge produces no JNI sync; under
system-enforced timeout semantics (process likely killed) this is acceptable.

## 8. Upstream Raw-Alchemy absorption follow-ups (adversarially adjudicated 2026-09)

Deferred items from the upstream comparison, post-adjudication: (a) hot-pixel median-filter fix
(upstream 44c2d21/1b0ad6d) is the only genuine algorithm gap — port a darktable-style Bayer-only
fix if long-exposure samples ever show stuck pixels; (b) when reviewing/merging the already
implemented `feat/rgb-denoise` submodule branch, run a σ-scan neutral-gray drift test on our own
x-veon models (upstream's σ≤0.5 clip is FastDenoise-v4-specific, not transferable) and skim upstream
`cc3871f` (v14 raw-main denoise) and `c982314` (pre7, touches 4 algorithm files) for reference;
(c) fold SHA-256(model bytes) into the QNN ctx cache key opportunistically next time `nn_session`
is touched (weights are compile-time embedded, so this is dev-workflow hardening, not a live bug);
(d) optional: latch NN run-failure after N consecutive failures in the grading service to avoid
paying a failed NN attempt per file in long batches. Session-idle unloading (upstream 3314f5c)
stays rejected: FastRPC teardown loops hang on SM8550 per our own code comments, and QNN HTP graph
memory lives in cDSP-side ion buffers that barely count toward app PSS.
