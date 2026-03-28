# Dedicated TS Bindings Exporter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace `cargo test`-driven ts-rs generation with a dedicated Rust bin exporter that works reliably in the normal build flow.

**Architecture:** Add a small `export-bindings` bin that calls `TS::export_all()` for every exported DTO and writes into `src-tauri/bindings`. Point `generate_ts_types()` at that bin and stop swallowing failures.

**Tech Stack:** Rust, ts-rs, Bash build scripts

---

### Task 1: Add failing verification for dedicated exporter path

**Files:**
- Modify: `scripts/build-common.sh`
- Test: `src/types/index.ts`

- [ ] **Step 1: Write the failing test**

Add a shell-level/source-level assertion that `generate_ts_types()` no longer uses `cargo test` for binding export and that the frontend guidance no longer tells developers to run `cargo test` for bindings.

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cmd.exe /C "cd /D D:\GitRepos\camera-ftp-companion\src-tauri && cargo.exe test --lib export_bindings"
```

Expected: FAIL on the old path and demonstrate why the new exporter is needed.

- [ ] **Step 3: Write minimal implementation**

Prepare the build/documentation path for the dedicated exporter.

- [ ] **Step 4: Run test to verify it passes**

Run the final generation/build commands after the bin exists.

### Task 2: Add dedicated Rust bindings exporter bin

**Files:**
- Create: `src-tauri/src/bin/export-bindings.rs`
- Modify: `src-tauri/src/lib.rs` (only if a re-export is needed)

- [ ] **Step 1: Write the failing test**

Attempt to run the new bin before it exists:

```bash
cmd.exe /C "cd /D D:\GitRepos\camera-ftp-companion\src-tauri && cargo.exe run --bin export-bindings"
```

Expected: FAIL because the bin does not exist yet.

- [ ] **Step 2: Write minimal implementation**

Implement a dedicated exporter that calls `TS::export_all()` for all current binding types:

- `AuthConfig`
- `AdvancedConnectionConfig`
- `ImageOpenMethod`
- `PreviewWindowConfig`
- `AndroidImageOpenMethod`
- `AndroidImageViewerConfig`
- `AppConfig`
- `ExifInfo`
- `FileInfo`
- `StorageInfo`
- `PermissionStatus`
- `ServerStartCheckResult`
- `ServerStateSnapshot`
- `ServerInfo`

- [ ] **Step 3: Run test to verify it passes**

Run:

```bash
cmd.exe /C "cd /D D:\GitRepos\camera-ftp-companion\src-tauri && cargo.exe run --bin export-bindings"
```

Expected: PASS and bindings are written.

### Task 3: Switch build scripts to the dedicated exporter and fail hard

**Files:**
- Modify: `scripts/build-common.sh`
- Modify: `src/types/index.ts`

- [ ] **Step 1: Write the failing test**

Run the normal generation/build path and confirm the old `cargo test` popup path is no longer required.

- [ ] **Step 2: Write minimal implementation**

Update `generate_ts_types()` to call `cargo run --bin export-bindings` and remove `|| true`. Update frontend comments to match the new command.

- [ ] **Step 3: Run test to verify it passes**

Run:

```bash
./build.sh windows android
```

Expected: PASS without relying on `cargo test` for bindings generation.
