# Dedicated TS Bindings Exporter Design

**Date:** 2026-03-28

**Goal:** Remove TypeScript binding generation's dependency on running the Windows Rust lib test binary by introducing a dedicated Rust bin command that exports ts-rs bindings directly.

---

## Problem Statement

The current build flow generates ts-rs bindings by running `cargo test` in `src-tauri`. On Windows this launches the GUI-linked lib test executable `camera_ftp_companion_lib-*.exe`, which fails to start with `STATUS_ENTRYPOINT_NOT_FOUND` (`TaskDialogIndirect`). This means normal builds can appear successful in the shell while still showing a blocking Windows popup and silently masking type-generation failure.

---

## Chosen Approach

Add a dedicated Rust bin, `src-tauri/src/bin/export-bindings.rs`, that directly calls `TS::export_all()` for all exported Rust DTOs. Update `scripts/build-common.sh` so `generate_ts_types()` calls `cargo run --bin export-bindings` instead of `cargo test` and fails hard on any export error.

This keeps ts-rs generation explicit, deterministic, and independent from test-binary startup on Windows.

---

## Scope

### In scope

- Add dedicated bindings exporter bin.
- Export every current `#[ts(export)]` type through that bin.
- Update build script to use the bin.
- Update docs/comments that still say “run cargo test” to regenerate bindings.

### Out of scope

- Fixing the Windows lib test `TaskDialogIndirect` loader issue itself.
- Refactoring the full ts-rs type layout.

---

## Success Criteria

1. `generate_ts_types()` no longer invokes `cargo test`.
2. `cargo run --bin export-bindings` generates all current bindings under `src-tauri/bindings`.
3. Build fails if bindings export fails.
4. `./build.sh windows android` no longer depends on the failing lib test binary startup path.
