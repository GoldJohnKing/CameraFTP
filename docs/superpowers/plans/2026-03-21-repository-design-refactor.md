# Repository Design Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eliminate the repository's main design risks around split sources of truth, cross-layer coupling, and oversized responsibilities without changing product behavior.

**Architecture:** Move runtime rules and configuration ownership into explicit backend services, reduce frontend stores/components to orchestration and presentation roles, and replace ad hoc cross-language strings with centralized protocol definitions. Execute in small, verifiable slices so Android permission flow, Windows tray behavior, and preview/gallery behavior remain working throughout the refactor.

**Tech Stack:** React, TypeScript, Zustand, Tauri v2, Rust, Kotlin/Android, ts-rs, TailwindCSS.

---

## Current State

- Android server-start prerequisites are effectively enforced in the frontend and Kotlin bridge instead of in the backend command path.
- Runtime config has multiple active facts: disk via repeated `AppConfig::load()`, frontend `configStore` draft state, and `AutoOpenService` in-memory cache.
- `serverStore`, `MainActivity`, `PreviewWindow`, and `GalleryCard` each combine several responsibilities and cross-cutting concerns.
- Tauri event names, JS bridge names, retry constants, and Android storage path values are duplicated across TypeScript, Rust, and Kotlin.
- Startup logic is split between `src/App.tsx` and feature components, especially `src/components/ConfigCard.tsx`.
- Several non-trivial failures are swallowed, leaving the app in potentially degraded but hard-to-diagnose states.

## Target State

- Backend commands own startup policy and configuration truth.
- Frontend stores consume backend policies and expose focused state/actions only.
- Cross-platform protocol identifiers live in a centralized contract layer.
- App startup is centralized in one bootstrap path.
- Android and preview/gallery logic are decomposed into smaller modules with narrower responsibilities.
- Error handling distinguishes recoverable silent degradation from important operational failures.

## Affected Files

| File | Change Type | Dependencies |
|------|-------------|--------------|
| `src-tauri/src/config.rs` | modify | blocks config service consumers |
| `src-tauri/src/lib.rs` | modify | manages new app state/services |
| `src-tauri/src/commands/config.rs` | modify | depends on config service |
| `src-tauri/src/commands/server.rs` | modify | depends on startup policy + config service |
| `src-tauri/src/commands/storage.rs` | modify | depends on permission snapshot/startup policy |
| `src-tauri/src/platform/android.rs` | modify | depends on permission snapshot contract |
| `src-tauri/src/platform/windows.rs` | modify | depends on config service and tray control refactor |
| `src-tauri/src/auto_open/service.rs` | modify | depends on config service |
| `src-tauri/src/file_index/service.rs` | modify | depends on config service |
| `src-tauri/src/ftp/server_factory.rs` | modify | depends on config service |
| `src-tauri/src/ftp/events.rs` | modify | event/protocol cleanup |
| `src-tauri/src/platform/traits.rs` | modify | richer platform policy interfaces |
| `src-tauri/src/platform/types.rs` | modify | depends on richer prerequisite/status types |
| `src-tauri/src/constants.rs` | modify | centralizes protocol/config constants |
| `src-tauri/src/commands/mod.rs` | modify | wire new command/service modules |
| `src-tauri/src/error.rs` | modify | add richer operational errors if needed |
| `src-tauri/src/config_service.rs` | create | created before most backend refactors |
| `src-tauri/src/startup_policy.rs` | create | created before server start refactor |
| `src-tauri/src/protocol.rs` | create | central event/bridge constant definitions |
| `src/stores/configStore.ts` | modify | depends on backend config authority |
| `src/stores/serverStore.ts` | modify | depends on startup policy result + extracted services |
| `src/stores/permissionStore.ts` | modify | depends on backend prerequisite truth |
| `src/App.tsx` | modify | depends on bootstrap extraction |
| `src/components/ConfigCard.tsx` | modify | depends on bootstrap cleanup |
| `src/components/PermissionDialog.tsx` | modify | permission/start contract changes |
| `src/components/ServerCard.tsx` | modify | permission/start contract changes |
| `src/components/PreviewWindow.tsx` | modify | depends on extracted hooks/services |
| `src/components/PreviewConfigCard.tsx` | modify | preview config propagation validation |
| `src/components/LatestPhotoCard.tsx` | modify as needed | event/protocol fallout |
| `src/components/GalleryCard.tsx` | modify | depends on extracted hooks/services |
| `src/utils/events.ts` | modify | depends on protocol/error policy |
| `src/utils/error.ts` | modify | error classification |
| `src/types/index.ts` | modify | shared binding re-export updates |
| `src/types/events.ts` | modify | depends on protocol centralization |
| `src/types/global.ts` | modify | bridge/type contract cleanup |
| `src/bootstrap/useAppBootstrap.ts` | create | central app initialization path |
| `src/services/server-events.ts` | create | extracted from server store |
| `src/services/android-service-sync.ts` | create | extracted from server store |
| `src/services/quit-flow.ts` | create | extracted from store/app glue |
| `src/hooks/usePreviewEvents.ts` | create | preview decomposition |
| `src/hooks/usePreviewNavigation.ts` | create | preview decomposition |
| `src/hooks/useZoomPan.ts` | create | preview decomposition |
| `src/hooks/useGallerySelection.ts` | create | gallery decomposition |
| `src/hooks/useGalleryThumbnails.ts` | create | gallery decomposition |
| `src/hooks/useGalleryActions.ts` | create | gallery decomposition |
| `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/MainActivity.kt` | modify | depends on extracted coordinators |
| `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/PermissionBridge.kt` | modify | depends on backend-owned policy |
| `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/FileUploadBridge.kt` | modify | depends on protocol/path centralization |
| `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/BridgeRegistry.kt` | create | Activity decomposition |
| `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/TauriEventRegistrar.kt` | create | Activity decomposition |
| `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/ForegroundServiceCoordinator.kt` | create | Activity decomposition |
| `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/BackPressCoordinator.kt` | create | Activity decomposition |
| `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/DeleteConfirmationCoordinator.kt` | create | Activity decomposition |
| `src/stores/__tests__/permissionStore.test.ts` | modify | permission/store contract changes |
| `src/components/__tests__/LatestPhotoCard.test.tsx` | modify as needed | event contract fallout |
| `src/utils/__tests__/*.test.ts` | modify/add | extracted hooks/services |
| `src-tauri/src/ftp/android_mediastore/tests.rs` | modify/add | backend contract changes |

## Execution Plan

### Phase 0: Safety Rails and Baseline

#### Task 0.1: Capture current behavior baseline

**Files:**
- Modify: none
- Test: existing project test/build commands only

- [ ] **Step 1: Record baseline commands**

Run:
```bash
npm test
./build.sh windows android
```

Expected:
- Existing tests pass or known failures are documented before refactor starts.
- Both platform builds complete successfully.

- [ ] **Step 2: Record critical manual flows**

Verify manually:
- Android permission request -> server start
- Windows tray start/stop/show/quit
- Preview window open, navigation, auto-front toggle
- Gallery refresh, selection, delete, share

- [ ] **Step 3: Create a refactor tracking note in the plan file**

Add a checklist section for observed baseline anomalies so later failures are not misattributed.

#### Task 0.2: Add focused characterization tests before moving logic

**Files:**
- Create: `src/stores/__tests__/serverStore.characterization.test.ts`
- Create: `src/components/__tests__/App.bootstrap.characterization.test.tsx`
- Modify: `src/stores/__tests__/permissionStore.test.ts`
- Create later in Phase 1: backend `#[cfg(test)]` blocks in new service modules

- [ ] **Step 1: Add characterization tests around current `serverStore` event registration and state transitions**
- [ ] **Step 2: Add characterization tests around current app bootstrap sequencing from `src/App.tsx`**
- [ ] **Step 3: Add tests for permission result normalization and prerequisite mapping in the current store shape**
- [ ] **Step 4: Run only the targeted tests**

Run:
```bash
npm test
./build.sh windows android
```

Expected: new tests pass and build remains green.

### Phase 1: Establish Backend Single Source of Truth

#### Task 1.1: Introduce a backend runtime config service

**Files:**
- Create: `src-tauri/src/config_service.rs`
- Modify: `src-tauri/src/config.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Define `ConfigService` around `Arc<RwLock<AppConfig>>` plus load/save/update APIs**
- [ ] **Step 2: Keep `AppConfig` as serialization model, not general runtime accessor**
- [ ] **Step 3: Register `ConfigService` in Tauri state during app startup**
- [ ] **Step 4: Add backend tests for initial load, update, persistence, and read-after-write consistency**
- [ ] **Step 5: Run `./build.sh windows android`**

Run:
```bash
./build.sh windows android
```

Expected: build passes with service wired in, even before all consumers migrate.

#### Task 1.2: Migrate backend consumers off ad hoc `AppConfig::load()`

**Files:**
- Modify: `src-tauri/src/commands/config.rs`
- Modify: `src-tauri/src/commands/server.rs`
- Modify: `src-tauri/src/auto_open/service.rs`
- Modify: `src-tauri/src/file_index/service.rs`
- Modify: `src-tauri/src/ftp/server_factory.rs`
- Modify: `src-tauri/src/platform/windows.rs`

- [ ] **Step 1: Update config commands to read/write through `ConfigService`**
- [ ] **Step 2: Update `AutoOpenService` to subscribe to or read from `ConfigService` instead of owning persistence**
- [ ] **Step 3: Update server factory, file index, and Windows storage path lookups to use `ConfigService`**
- [ ] **Step 4: Remove remaining non-bootstrap `AppConfig::load()` call sites or document why a bootstrap-only use remains**
- [ ] **Step 5: Re-run affected tests and `./build.sh windows android`**

Verify with search:
```bash
rg "AppConfig::load\(" src-tauri/src
```

Expected: only bootstrap or explicitly justified uses remain.

#### Task 1.3: Refactor frontend config flow to treat backend as authority

**Files:**
- Modify: `src/stores/configStore.ts`
- Modify: `src/components/ConfigCard.tsx`

- [ ] **Step 1: Preserve draft editing locally but refresh persisted state from backend save responses**
- [ ] **Step 2: Ensure debounced saves cannot leave `config` and `draft` permanently divergent**
- [ ] **Step 3: Add tests for draft revision conflict handling and rollback behavior**
- [ ] **Step 4: Run targeted tests and `./build.sh windows android`**

### Phase 2: Move Android Permission Snapshot and Startup Policy Ownership Into Backend

#### Task 2.1: Define Android permission snapshot transport first

**Files:**
- Modify: `src-tauri/src/platform/types.rs`
- Modify: `src-tauri/src/platform/traits.rs`
- Modify: `src-tauri/src/commands/storage.rs`
- Modify: `src-tauri/src/platform/android.rs`
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/PermissionBridge.kt`
- Modify: `src/stores/permissionStore.ts`

- [ ] **Step 1: Define a backend-owned permission snapshot shape that Rust, TS, and Kotlin can all agree on**
- [ ] **Step 2: Add or update the storage/permission command path so Rust can obtain real Android permission facts instead of placeholders**
- [ ] **Step 3: Update Kotlin `PermissionBridge` to expose only raw permission snapshot data needed by the backend/TS layers**
- [ ] **Step 4: Update `permissionStore` to consume the explicit snapshot contract rather than infer policy from bridge availability**
- [ ] **Step 5: Define the snapshot shape as a Rust-owned `#[derive(TS)]` contract, run `./build.sh gen-types`, and update `src/types/index.ts` before frontend consumers are changed**
- [ ] **Step 6: Add targeted tests for denied, partial, and granted permission snapshots**
- [ ] **Step 7: Verify with `./build.sh windows android` before moving startup enforcement**

#### Task 2.2: Create a backend startup policy module

**Files:**
- Create: `src-tauri/src/startup_policy.rs`
- Modify: `src-tauri/src/platform/types.rs`
- Modify: `src-tauri/src/platform/android.rs`
- Modify: `src-tauri/src/commands/server.rs`
- Modify: `src-tauri/src/commands/storage.rs`

- [ ] **Step 1: Define a structured prerequisite model with explicit failure reasons**
- [ ] **Step 2: Make `start_server` enforce startup policy before server creation**
- [ ] **Step 3: Return structured errors/results that frontend can render without re-implementing rules**
- [ ] **Step 4: Define the startup-policy result as a Rust-owned `#[derive(TS)]` contract, run `./build.sh gen-types`, and update `src/types/index.ts` before frontend consumers are changed**
- [ ] **Step 5: Add backend tests for denied, partial, and granted permission/start conditions**
- [ ] **Step 6: Verify with `./build.sh windows android`**

#### Task 2.3: Reduce frontend permission/start duplication

**Files:**
- Modify: `src/stores/serverStore.ts`
- Modify: `src/stores/permissionStore.ts`
- Modify: `src/components/PermissionDialog.tsx`
- Modify: `src/components/ServerCard.tsx`

- [ ] **Step 1: Change frontend start flow to request backend prerequisites rather than decide independently**
- [ ] **Step 2: Keep permission store focused on permission acquisition status, not startup eligibility policy**
- [ ] **Step 3: Ensure UI still shows guided permission dialogs using backend reasons**
- [ ] **Step 4: Add/adjust tests for permission dialog and server start behavior**
- [ ] **Step 5: Verify Android flow manually and via build**

#### Task 2.4: Simplify Kotlin permission bridge responsibilities

**Files:**
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/PermissionBridge.kt`

- [ ] **Step 1: Limit bridge to raw permission querying/requesting and Android-specific intents only if any non-raw workflow logic still remains after Task 2.1**
- [ ] **Step 2: Remove any hidden business-policy assumptions from the bridge API**
- [ ] **Step 3: Keep return payloads small and explicit so Rust/TS can consume them consistently**
- [ ] **Step 4: Verify with `./build.sh windows android`**

### Phase 3: Centralize Cross-Language Protocols and Constants

#### Task 3.1: Introduce a shared protocol definition layer

**Files:**
- Create: `src-tauri/src/protocol.rs`
- Modify: `src-tauri/src/constants.rs`
- Modify: `src/types/events.ts`
- Modify: `src/utils/events.ts`
- Modify: `src/types/global.ts`

- [ ] **Step 1: Move Tauri event names, key bridge identifiers, and retry-related constants into explicit protocol modules for Rust and TypeScript**
- [ ] **Step 2: Add `ProtocolConstants.kt` as the single Kotlin mirror file for Android-side protocol names; all Kotlin bridge/activity code must import from it instead of hardcoding strings**
- [ ] **Step 3: Replace free-form string usage at high-value call sites with imported constants**
- [ ] **Step 4: Any Rust-owned shared contract introduced or changed in this phase must derive `TS`; run `./build.sh gen-types` and update `src/types/index.ts` before frontend usage changes**
- [ ] **Step 5: Add frontend tests for event registration helpers using the new constants**
- [ ] **Step 6: Verify no behavior change in event subscriptions and run `./build.sh windows android`**

#### Task 3.2: Remove duplicated Android path/config facts

**Files:**
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/FileUploadBridge.kt`
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/MainActivity.kt`
- Modify: `src-tauri/src/platform/android.rs`

- [ ] **Step 1: Replace hardcoded storage path and listener constants with centralized protocol/config constants; use explicit runtime injection from the single Kotlin mirror file where a runtime value is required**
- [ ] **Step 2: Remove comments that warn constants must manually match other languages by making them actually shared**
- [ ] **Step 3: Verify file upload scanning and Tauri listener registration still work**
- [ ] **Step 4: Run `./build.sh windows android`**

### Phase 4: Centralize App Bootstrap and Lifecycle

#### Task 4.1: Extract a single frontend bootstrap path

**Files:**
- Create: `src/bootstrap/useAppBootstrap.ts`
- Modify: `src/App.tsx`
- Modify: `src/components/ConfigCard.tsx`

- [ ] **Step 1: Move platform/config/permission/listener startup to `useAppBootstrap`**
- [ ] **Step 2: Keep `App.tsx` focused on layout and top-level dialogs**
- [ ] **Step 3: Remove duplicate `loadConfig()` / `loadPlatform()` calls from `ConfigCard`**
- [ ] **Step 4: Add test coverage for bootstrap sequencing where practical**
- [ ] **Step 5: Verify app still initializes correctly on both platforms**

#### Task 4.2: Move tray-control ownership to the backend before extracting server event services

**Files:**
- Modify: `src-tauri/src/platform/windows.rs`
- Modify: `src-tauri/src/commands/server.rs`
- Modify: `src/stores/serverStore.ts`

- [ ] **Step 1: Change tray start/stop to call backend orchestration directly from `src-tauri/src/platform/windows.rs`**
- [ ] **Step 2: Convert frontend server event extraction to observer-only semantics; frontend no longer owns tray command routing**
- [ ] **Step 3: Manually verify Windows tray start/stop/show/quit behavior**
- [ ] **Step 4: Run `./build.sh windows android`**

#### Task 4.3: Separate quit/tray/window orchestration from server state store

**Files:**
- Create: `src/services/quit-flow.ts`
- Create: `src/services/server-events.ts`
- Create: `src/services/android-service-sync.ts`
- Modify: `src/stores/serverStore.ts`
- Modify: `src/App.tsx`

- [ ] **Step 1: Extract Tauri event registration from `serverStore`**
- [ ] **Step 2: Extract Android service sync logic from `serverStore`**
- [ ] **Step 3: Extract quit dialog trigger flow from store/app coupling into a small service**
- [ ] **Step 4: Leave `serverStore` responsible only for state + commands**
- [ ] **Step 5: Add targeted tests for the extracted services**
- [ ] **Step 6: Run `./build.sh windows android`**

### Phase 5: Decompose Oversized Frontend Components

#### Task 5.1: Refactor preview window into focused hooks/modules

**Files:**
- Create: `src/hooks/usePreviewEvents.ts`
- Create: `src/hooks/usePreviewNavigation.ts`
- Create: `src/hooks/useZoomPan.ts`
- Modify: `src/components/PreviewWindow.tsx`

- [ ] **Step 1: Extract preview event subscription/loading behavior**
- [ ] **Step 2: Extract file navigation and index recovery logic**
- [ ] **Step 3: Extract zoom/pan/fullscreen interaction logic**
- [ ] **Step 4: Keep `PreviewWindow.tsx` primarily as composition + rendering**
- [ ] **Step 5: Add hook-level tests for navigation and zoom invariants**
- [ ] **Step 6: Manually verify preview open, navigation, fullscreen, and auto-front toggle**
- [ ] **Step 7: Run `./build.sh windows android`**

#### Task 5.2: Refactor gallery card into focused hooks/modules

**Files:**
- Create: `src/hooks/useGallerySelection.ts`
- Create: `src/hooks/useGalleryThumbnails.ts`
- Create: `src/hooks/useGalleryActions.ts`
- Modify: `src/components/GalleryCard.tsx`
- Modify: `src/utils/gallery-delete.ts`
- Modify: `src/utils/media-store-events.ts`

- [ ] **Step 1: Extract selection mode and Android back-press coordination**
- [ ] **Step 2: Extract thumbnail lazy-loading/cache cleanup logic**
- [ ] **Step 3: Extract delete/share/refresh action logic**
- [ ] **Step 4: Keep `GalleryCard.tsx` focused on rendering and wiring**
- [ ] **Step 5: Add tests around selection transitions, refresh preconditions, and delete failure messaging**
- [ ] **Step 6: Manually verify gallery refresh, selection, delete, share, and built-in viewer opening**
- [ ] **Step 7: Run `./build.sh windows android`**

### Phase 6: Decompose Android Activity and Bridges

#### Task 6.1: Split `MainActivity` into coordinators with lifecycle-safe boundaries

**Files:**
- Create: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/BridgeRegistry.kt`
- Create: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/TauriEventRegistrar.kt`
- Create: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/ForegroundServiceCoordinator.kt`
- Create: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/BackPressCoordinator.kt`
- Create: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/DeleteConfirmationCoordinator.kt`
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/MainActivity.kt`

- [ ] **Step 1: Keep `registerForActivityResult` ownership in `MainActivity` or an Activity-bound lifecycle helper; do not move it into a plain utility object**
- [ ] **Step 2: Move bridge setup/registration out of `MainActivity` where no lifecycle registration is required**
- [ ] **Step 2: Move Tauri listener retry logic into `TauriEventRegistrar`**
- [ ] **Step 3: Move foreground service start/update/stop logic into `ForegroundServiceCoordinator`**
- [ ] **Step 4: Move selection-mode back-press behavior into `BackPressCoordinator`**
- [ ] **Step 5: Move delete confirmation latch handling into an Activity-bound `DeleteConfirmationCoordinator` that receives launcher callbacks from `MainActivity`**
- [ ] **Step 6: Keep `MainActivity` as composition + lifecycle shell**
- [ ] **Step 7: Verify Android build and critical manual flows**
- [ ] **Step 8: Run `./build.sh windows android`**

#### Task 6.2: Keep bridges focused and reusable

**Files:**
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/FileUploadBridge.kt`
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/GalleryBridge.kt`
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/ServerStateBridge.kt`
- Modify as needed: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/ImageViewerBridge.kt`

- [ ] **Step 1: Ensure each bridge has one narrow purpose and consumes shared coordinators/helpers instead of owning workflow logic**
- [ ] **Step 2: Remove path/policy duplication from bridges**
- [ ] **Step 3: Verify bridge JS APIs remain backward compatible or update frontend call sites atomically**
- [ ] **Step 4: Run `./build.sh windows android`**

### Phase 7: Improve Error Semantics and Operational Visibility

#### Task 7.1: Classify silent vs reportable failures

**Files:**
- Modify: `src/utils/events.ts`
- Modify: `src/utils/error.ts`
- Modify: `src/App.tsx`
- Modify: `src/stores/serverStore.ts`
- Modify: `src/components/PreviewWindow.tsx`
- Modify: `src/components/GalleryCard.tsx`

- [ ] **Step 1: Define error-handling rules for initialization, event registration, optional metadata, and user preference operations**
- [ ] **Step 2: Replace blanket silent catches in important paths with structured warnings or surfaced errors**
- [ ] **Step 3: Leave clearly optional flows silent only when intentionally documented**
- [ ] **Step 4: Add tests around event registration failure handling where practical**
- [ ] **Step 5: Run `./build.sh windows android`**

#### Task 7.2: Harden tray-control and lifecycle observability after the Phase 4 decision

**Files:**
- Modify: `src-tauri/src/platform/windows.rs`
- Modify: `src/stores/serverStore.ts`
- Modify: `src/utils/events.ts`

- [ ] **Step 1: Add explicit diagnostics around the backend-direct tray-control path chosen in Phase 4**
- [ ] **Step 2: Ensure failures in tray event registration or backend invocation are observable in logs**
- [ ] **Step 3: Re-run manual Windows tray checks**
- [ ] **Step 4: Run `./build.sh windows android`**

### Phase 8: Cleanup, Search-Based Validation, and Final Verification

#### Task 8.1: Remove obsolete duplication and dead abstractions

**Files:**
- Modify/delete only after migration confirms no active references remain

- [ ] **Step 1: Remove deprecated helper paths replaced by services/hooks/coordinators**
- [ ] **Step 2: Search for old event names, old bridge names, and duplicate path constants**
- [ ] **Step 3: Remove unused imports and stale comments that describe no-longer-true behavior**

Run:
```bash
rg "tray-start-server|tray-stop-server|android-open-manage-storage-settings|android-open-storage-permission-settings|preview-image|preview-config-changed|window-close-requested|server-started|server-stopped|stats-update|file-index-changed|FileUploadAndroid|PermissionAndroid|/storage/emulated/0/DCIM/CameraFTP|TAURI_LISTENER_MAX_RETRIES|TAURI_LISTENER_RETRY_DELAY_MS" src src-tauri
```

Expected: either no matches remain or only centralized definitions remain.

#### Task 8.2: Full verification

**Files:**
- Modify: none

- [ ] **Step 1: Run frontend tests**
- [ ] **Step 2: Run full cross-platform build**
- [ ] **Step 3: If any ts-rs shared type changes were introduced, regenerate bindings as part of verification**
- [ ] **Step 4: Run manual smoke checks for Android permission, Windows tray, preview, gallery, close-to-tray, autostart hidden-window, gallery back-press, delete confirmation, and preview-config propagation flows**

Run:
```bash
npm test
./build.sh gen-types
./build.sh windows android
```

Expected: tests and builds pass; critical user flows behave like baseline or better.

## Rollback Plan

If something fails:

1. Revert the current phase only; do not mix rollback across multiple phases.
2. Restore original runtime entry points first (`src-tauri/src/lib.rs`, `src/App.tsx`, `src/stores/serverStore.ts`, `src-tauri/gen/android/.../MainActivity.kt`).
3. Keep characterization tests added in Phase 0 unless they were wrong; they are safety rails, not refactor artifacts.
4. If config-service migration causes instability, temporarily keep `ConfigService` as a thin wrapper over legacy disk-backed access while preserving the new API surface.
5. If Android permission policy migration stalls, keep backend prerequisite types but adapt them to current Kotlin checks rather than reverting the entire policy contract.

## Risks

- Android permission/startup flow can regress because policy currently spans Rust, TS, and Kotlin. Mitigation: migrate with characterization tests and manual device checks after every phase.
- Config migration can introduce stale-state or persistence regressions. Mitigation: backend service tests plus search-based elimination of `AppConfig::load()`.
- Decomposing `PreviewWindow` and `GalleryCard` can break subtle UI interactions. Mitigation: extract logic behind hooks first, keep JSX structure stable initially.
- `MainActivity` decomposition can break bridge lifecycle timing. Mitigation: keep registration order unchanged and add focused coordinator tests/logging where possible.
- Tray refactor can change Windows control semantics. Mitigation: manual tray smoke tests before and after.

## Recommended Commit Sequence

- `test: add characterization coverage for refactor baseline`
- `refactor(backend): introduce runtime config service`
- `refactor(server): move startup policy into backend`
- `refactor(protocol): centralize cross-layer event and bridge contracts`
- `refactor(app): centralize frontend bootstrap`
- `refactor(store): split server orchestration from server state`
- `refactor(preview): extract preview hooks and navigation services`
- `refactor(gallery): extract gallery state and action hooks`
- `refactor(android): split main activity coordinators`
- `refactor(errors): classify silent vs surfaced operational failures`
- `refactor(cleanup): remove obsolete duplication and verify`
