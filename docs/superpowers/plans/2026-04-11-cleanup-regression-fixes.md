# Cleanup Regression Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the cleanup branch regressions in preview auto-bring-to-front state syncing and config draft/backend merge behavior, while removing low-value dead-code/source-guard tests added during cleanup.

**Architecture:** Keep the cleanup refactors, but restore behavior at the boundaries where simplification changed state flow. Add focused regression tests for the actual runtime behavior, then remove brittle source-text tests that do not protect user-visible functionality.

**Tech Stack:** React, Zustand, Vitest, TypeScript, Kotlin unit tests, Tauri

---

### Task 1: Reproduce the regressions with tests

**Files:**
- Modify: `src/stores/__tests__/configStore.test.ts`
- Modify: `src/components/PreviewWindow.tsx` or add a focused component test if needed

- [ ] Add a failing config-store regression test proving that a local auth-only draft change must not preserve a stale `advancedConnection.enabled` value after backend resync.
- [ ] Add a failing PreviewWindow regression test proving that a later `autoBringToFront` prop change updates the local toggle state.
- [ ] Run the focused tests and confirm they fail for the expected reasons.

### Task 2: Implement the minimal fixes

**Files:**
- Modify: `src/stores/configStore.ts`
- Modify: `src/components/PreviewWindow.tsx`

- [ ] Restore prop-to-local sync for `localAutoBringToFront`.
- [ ] Change `advancedConnection` merge logic back to field-level dirty preservation.
- [ ] Re-run the focused tests and confirm they pass.

### Task 3: Remove low-value cleanup tests

**Files:**
- Delete: `src/hooks/__tests__/usePreviewConfigListener.test.ts`
- Delete: `src/types/__tests__/index.exports-source-guard.test.ts`
- Delete: `src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/bridges/GalleryBridgeDeadOverloadTest.kt`
- Delete: `src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/bridges/ImageViewerBridgeDeadCodeTest.kt`
- Delete: `src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/galleryv2/ThumbnailCacheV2DeadCodeTest.kt`
- Delete: `src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/galleryv2/ThumbnailPipelineDeadCodeTest.kt`

- [ ] Remove the brittle source-text tests introduced by the cleanup work.
- [ ] Ensure remaining behavior tests still cover the intended runtime paths.

### Task 4: Verify

**Files:**
- No code changes expected

- [ ] Run the targeted Vitest suite for changed TS tests.
- [ ] Run the required project verification command: `./build.sh windows android`.
- [ ] Review the output before making any completion claim.
