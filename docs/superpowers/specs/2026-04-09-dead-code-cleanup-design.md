# Dead Code Cleanup Design

**Date:** 2026-04-09
**Scope:** Remove dead code and simplify redundant patterns across Rust and TypeScript

---

## Dead Code Removals

### R1: `get_certificate_paths()` — `crypto/tls.rs:154-160`

**What:** `pub fn get_certificate_paths() -> AppResult<CertificatePaths>` — returns cert paths without checking validity.

**Why dead:** Never called anywhere. The codebase uses `ensure_certificate_paths()` (same file, ~line 110) which both resolves paths AND creates certs if missing.

**Change:** Delete lines 153-160.

### R2: `AppConfig::load()` — `config.rs:250-285`

**What:** `pub fn load() -> Self` — reads config from JSON file with fallback to default.

**Why dead:** Never called. All config loading goes through `ConfigService::load()` / `ConfigService::load_from_path()` which provides the same functionality with proper error handling.

**Change:** Delete lines 250-285 (the entire method).

### R3: `TransientEventBus` — `ftp/types.rs:345-371`

**What:** A `#[allow(dead_code)]` struct + impl that provides a non-persisting event bus for tests.

**Why test-only:** Only used inside `#[cfg(test)]` modules in `ftp/events.rs` (lines 750, 800). Currently sits in production code with a dead_code suppression.

**Change:** Move the entire `TransientEventBus` definition (struct + Default impl + impl block) inside the existing `#[cfg(test)]` module at the bottom of `ftp/types.rs`, or into a `test_utils` module gated by `#[cfg(test)]`.

### R4: `get_file_count()` — `file_index/service.rs:469-472`

**What:** `pub async fn get_file_count(&self) -> usize` — returns file count from the index.

**Why test-only:** Has `#[allow(dead_code)]`, only called in test assertions within the same file (lines 538, 553).

**Change:** Move method behind `#[cfg(test)]` gate.

### T1: Unused gallery-v2 re-exports — `types/index.ts:31-39`

**What:** 6 re-exports from `./gallery-v2` in the barrel file: `MediaPageRequest`, `MediaPageResponse`, `MediaCursor`, `ThumbRequest`, `ThumbResult`, `ThumbResultListener`.

**Why dead:** All consumers import directly from `'../types/gallery-v2'`. The barrel re-exports add no value.

**Change:** Remove lines 30-39 from `types/index.ts` (the entire gallery-v2 re-export section + comment).

---

## Code Simplifications

### T2: Pointless try/catch — `configStore.ts:197-203`

**What:** `setAutostart` wraps `invoke()` in `try { ... } catch(e) { throw e; }`.

**Change:** Remove try/catch wrapper, call `invoke()` directly.

### T3: Duplicate `setShowMenu(false)` — `useGallerySelection.ts:156`

**What:** `setShowMenu(false)` is called at line 140 (start of `handleDelete`) and again at line 156 (early return) and line 189 (before animation).

**Change:** Remove the call at line 156 (inside the `if (!resultJson)` early return), since it's redundant with line 140. Keep the one at line 189 as it serves a different purpose (ensuring menu is closed before animation).

### T4: Unnecessary `...state` spreads — `serverStore.ts:106-125`

**What:** `setServerRunning`, `setServerStopped`, `setServerStats` use `set((state) => ({ ...state, ... }))` but explicitly set all relevant fields.

**Change:** Replace with `set({ ... })` (remove the callback and spread).

### T5: Redundant prop-to-state sync — `PreviewWindow.tsx:94-96`

**What:** A `useEffect` syncs `localAutoBringToFront` from the `autoBringToFront` prop. The local state also gets updated from event listeners, which is the real purpose. But the prop sync is redundant because prop changes already trigger re-renders.

**Change:** Remove the `useEffect` sync at lines 94-96. Keep the initial `useState(autoBringToFront)` which correctly seeds the local state.

---

## Test Strategy

Since this is cleanup (deletion + simplification), tests serve as regression guards:

1. **Before changes:** Run existing test suites to establish baseline
2. **Rust changes:** Existing Rust tests must continue passing. For R3/R4, moving code behind `#[cfg(test)]` means test code still compiles and runs unchanged.
3. **TypeScript changes:** Existing Vitest tests must continue passing. The guard test in `types/__tests__/` explicitly asserts certain types are NOT re-exported — removing the gallery-v2 re-exports is consistent with that guard's intent.
4. **Build verification:** Run `./build.sh windows android` to verify no compilation errors.

## Risk Assessment

| Item | Risk | Reason |
|------|------|--------|
| R1 | None | Function never called |
| R2 | None | Method never called |
| R3 | Low | Only moves existing code behind test gate |
| R4 | Low | Only moves existing code behind test gate |
| T1 | Low | Re-exports never consumed via barrel |
| T2 | None | Pure simplification, same behavior |
| T3 | Low | Removing redundant call |
| T4 | None | Zustand `set()` without callback replaces all listed keys |
| T5 | Low | Removing redundant sync, state still initialized from prop |
