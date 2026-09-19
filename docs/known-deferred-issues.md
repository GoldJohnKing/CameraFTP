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

## 8. Upstream Raw-Alchemy absorption follow-ups (adversarially adjudicated 2026-09; item b completed 2026-09-19)

Remaining deferred items: (a) hot-pixel median-filter fix (upstream 44c2d21/1b0ad6d) is the
only genuine algorithm gap — upstream's `fix_hot_pixels` is CFA-pattern-generic (Bayer 2×2 and
X-Trans 6×6), port if long-exposure samples ever show stuck pixels; (c) fold SHA-256(model bytes)
into the QNN ctx cache key opportunistically next time `nn_session` is touched (three models are
now compile-time embedded — bayer, xtrans, fastdenoise — so this is dev-workflow hardening, not a
live bug); (d) optional: latch NN run-failure after N consecutive failures in the grading service
to avoid paying a failed NN attempt per file in long batches. Session-idle unloading (upstream
3314f5c) stays rejected: FastRPC teardown loops hang on SM8550 per our own code comments, and QNN
HTP graph memory lives in cDSP-side ion buffers that barely count toward app PSS.

Completed (b) 2026-09-19: `feat/rgb-denoise` merged into the submodule (RGB-domain FastDenoise v4
as the sole NN-variant denoise; raw-domain denoise disabled by default in neural builds, legacy
variant bit-identical to before). σ-scan neutral-grey drift test executed — see §12 for the
findings that changed the shipped default. Upstream `cc3871f`/`c982314` skimmed as directed:
c982314 (pre7) is session/memory engineering only (free-dim override, stale-session latch, chunked
61MP accumulation) with zero algorithm delta; cc3871f is the dormant unshipped v14 raw-domain
model path. Pipeline placement fixed to upstream op order (denoise first, lens second — submodule
d4e1c77); C++↔upstream parity 1.34e-4 across all σ.

## 10. Processing FGS: androidx ServiceCompat masks out mediaProcessing (type-none crash)

`ServiceCompat.startForeground` (androidx.core, Api34Impl on API 34+) ANDs the requested type with
`FOREGROUND_SERVICE_TYPE_ALLOWED_SINCE_U`, a compile-time mask that still does not include
`FOREGROUND_SERVICE_TYPE_MEDIA_PROCESSING` (verified against androidx-main, 2026-09). Requesting
mediaProcessing through ServiceCompat therefore reaches the platform as type 0, and with
targetSdk >= 34 AMS throws `InvalidForegroundServiceTypeException "Starting FGS with type none"`
— this crashed the app on the service's first real-device run (Xiaomi ishtar, Android 16).
`ProcessingForegroundService` now calls the platform `startForeground` directly with a
mediaProcessing → dataSync fallback ladder. Once a future androidx release adds mediaProcessing
to the mask, the ladder can collapse back to a single ServiceCompat call. Note the dataSync rung
inherits the 6h/24h FGS quota (targetSdk 35+); `onTimeout` is already implemented.

## 11. AI-edit gallery enqueue still resolves file paths one-by-one

`useGallerySelection.ts` (~:285) still uses the per-file `resolveFilePath` bridge loop that made
batch color grading's FGS notification lag ~7s behind task creation (measured tap→enqueue on a
real device; fixed for color grading via the batch `resolveFilePaths` bridge in
`ImageViewerBridge`). Migrate it to `resolveFilePaths` when AI-edit batch enqueue UX is next
touched — the bridge, TS declaration, and receiver-bound invocation pattern are already in place
from the color-grading fix.

## 9. ndk-context self-initialization workaround (tauri 2.11.x / tao 0.35.3 regression)

**Root cause**: the a5458cb dependency-stack upgrade (tauri 2.11.5 → tauri-runtime-wry 2.11.4 →
tao 0.35.3) pulled an upstream regression where tao no longer initializes
[ndk-context](https://crates.io/crates/ndk-context) (tao#1220/#1266; fixed in tao 0.36, which
tauri 2.12 — unreleased at fix time — will pick up). Every Rust→JNI call goes through
`src-tauri/src/utils/jni.rs` `java_vm()`/`android_context()` → `ndk_context::android_context()`,
which panics when uninitialized. **Symptoms**: FTP MediaStore bridge fully broken (LIST silently
returns an empty listing, STOR fails with 550) and Android service-state sync dead (neither FGS
notification ever appears).

**Fix**: the app now initializes ndk-context itself — `MainActivity.onCreate` calls
`initNdkContext(applicationContext)` immediately after `super.onCreate()` (the native library is
already loaded inside `super.onCreate` via `WryActivity.onCreate → Rust.onActivityCreate →
System.loadLibrary`). The Rust handler
`Java_com_gjk_cameraftpcompanion_MainActivity_initNdkContext` in `utils/jni.rs` stores a
process-lifetime `GlobalRef` of the Application context (never deleted — ndk-context keeps the
raw pointer forever) plus the `JavaVM` raw pointer, guarded by a `std::sync::Once` for activity
re-creation and a `catch_unwind` around `initialize_android_context` (ndk-context 0.1.1 asserts
on double-init; first initializer wins, later one is harmlessly ignored — and because 0.1.1
replaces-then-asserts, the keepalive ref is retained on both paths). The
`java_vm()`/`android_context()` helpers additionally convert the uninitialized-panic into a
guided `AppError` ("MainActivity.initNdkContext must run first").

**Removal condition**: delete the Kotlin declaration + onCreate call + the Rust init block
(keepalive statics, `init_ndk_context`, `run_ndk_context_init`, the JNI entrypoint, and the
AGENTS.md pitfall entry) once tauri ≥ 2.12 ships (or the resolved stack contains tao ≥ 0.36).
The `catch_unwind` makes coexistence safe, but the workaround must not outlive its upstream fix.

## 12. RGB denoise (FastDenoise v4): σ-dependent chroma drift and calibration limits (2026-09-19)

The merged RGB denoise (NN variant's sole denoise; legacy variant keeps raw-domain) has a
σ-dependent, non-monotonic neutral-grey drift on warm scenes — measured on the tungsten-lit
Sample.NEF through the shipped pipeline: B/G drift +1.16% @σ0.01, +2.94% @0.05, +7.50% @0.10,
+11.81% @0.25 (upstream's own default — the worst point), +9.57% @0.35, +3.49% @0.5 (R/G stays
within ±3.5% throughout). C++↔upstream parity is 1.34e-4 at every σ, so this is the upstream
model's own chroma behavior, not a porting bug; upstream never characterized the mid-σ range on
real warm samples (their σ≤0.5 clip came from the σ>0.5 green-shift boundary only). Shipped
default is therefore 0.05 (`NN_RGB_DENOISE_STRENGTH` in `color_grading/ffi.rs`), inside the
verified-safe zone with margin against the ±5% gate.

Calibration limits (revisit when conditions change): (a) single-sample data — one warm Bayer NEF,
no X-Trans RAF exists in `Test/` (harness takes `--sample`, auto-selects the X-Trans path);
re-run the σ-scan when an RAF lands; (b) cross-validation runs CPU EP on both sides — the
Android QNN HTP fp16 denoise path has no on-device parity smoke yet (compare a dump vs CPU
reference at ~1e-2 tolerance when convenient); (c) if upstream re-trains FastDenoise, re-vendor
bit-identical bytes (`resources/models/fastdenoise/`) and re-run the σ-scan — parity numbers are
only meaningful against exact upstream bytes; (d) `cfa_residue_suppression` (period-2 residue
elimination after x-veon NN demosaic) runs unconditionally, costing −6dB at period-4 detail even
with denoise off — a chroma-only or energy-gated refinement is the follow-up if fine texture
softening is ever reported.

## 13. Android exit-path: QNN HTP static-destructor crash; legacy variant clean-build fragility (2026-09-19)

Two issues found while verifying the NN-variant denoise on a real device (Xiaomi ishtar,
SM8550, Android 16):

1. **Exit-time SIGABRT after QNN use** — every app close after QNN graphs were initialized
   crashed in `exit → __cxa_finalize → libQnnHtpPrepare.so GraphPrepare::~GraphPrepare`
   (Scudo "invalid chunk state"; observed 2026-09-18/19 across pre- and post-merge builds,
   so not caused by the FastDenoise merge). Root cause chain: tao 0.35.3's Android backend
   terminates the process via `std::process::exit` when its event loop ends
   (`platform_impl/android/mod.rs`, `EventLoop::run`) → libc exit runs QNN's static
   destructors → QNN/Scudo teardown bug (in-process FastRPC teardown on this SoC is also
   known to hang — §8). Mitigation: `-Wl,--wrap=exit` (build.rs, Android only) routes every
   `exit()` from this library to `__wrap_exit` (`src/platform/android.rs`), which flushes
   stdio and `_exit()`s directly — no atexit, no `__cxa_finalize`, no QNN/FastRPC
   destructors; the kernel reclaims everything as a SIGKILL would. **Removal condition**:
   drop the link arg + `__wrap_exit` when tao/tauri stop terminating Android via libc
   `exit`, or QNN fixes the destructor; revisit on tauri ≥ 2.12 / tao ≥ 0.36 upgrades.
2. **Legacy-variant clean-build breakage (fixed)** — `color_grading/resources.rs` had no
   `cfg(nn_demosaic)` gating despite build.rs documenting stubs: legacy builds compiled the
   `include_bytes!` of `OUT_DIR/nn_models/*.gz` only because a previous NN build's files
   lingered in the shared OUT_DIR — a clean legacy build would fail to compile, and legacy
   APKs silently embedded ~29 MB of dead model bytes. Now properly gated with
   `cfg(not(nn_demosaic))` stubs returning `None` (models are neither embedded nor
   referenced in legacy builds).
