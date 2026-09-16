# 批次 2：专项加固 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to execute this plan. Work through tasks sequentially; do not skip the failing-test-first steps; commit after each green step with the exact messages below.

## Goal

针对对抗性审查确认成立的 4 个问题落地修复：

1. **安全纵深**：`image-preview` 自定义 scheme 无路径校验（任意路径可读，含越界）、`std::thread::spawn` 每请求一线程无上限、assetProtocol 静态 scope 为 `["**"]`、CSP 为 `null`。
2. **CI 缺失**：仓库无 `.github/`（已核实），需要最小 GitHub Actions 流水线。
3. **图库重渲染链**：hook 返回裸对象字面量 + effect 依赖引用不稳，导致缩略图批量到达期间 O(n) `registerMedia` 重跑与 range 上报反复重置 60ms debounce（突发期有限延迟，非无限饥饿）。
4. **索引扫描**：逐文件串行 `await get_file_info`（EXIF 经 spawn_blocking 但串行）；`set_files` 无条件整体覆盖，FTP Put 通道（`listeners.rs:81-114` 独立 spawn 调 `add_file`）可在扫描 readdir 与提交之间插入导致该文件从索引丢失。watcher 通道竞态已被 stop-first 设计消除（`update_save_path` 先 `stop_watcher`），不在本批范围。

## Architecture

Tauri v2（2.11.5）单仓库：React 18 + TS + Zustand + Tailwind 前端（`src/`），Rust 后端（`src-tauri/`，lib crate `camera_ftp_companion_lib`），Android 侧 `src-tauri/gen/android/`（含 Kotlin 测试），C++ 子模块 `src-tauri/lib/rawalchemy`。构建统一入口 `./build.sh windows android`（WSL2 内通过 `cargo.exe` 交叉产出 Windows 产物）。本批次改动不新增模块，只改：scheme handler（lib.rs）、配置提交（commands/config.rs）、file_index 扫描/提交（service.rs）、3 个前端文件、1 个新 CI workflow。

**对话内核实报告（关键事实，均已读源码确认；接口经发起方复核：`get_or_default` config_service.rs:68、泛型 `mutate_and_persist<F, R>` :78、`resizeMock.triggerResize` 测试基建存在）**：

- `image-preview` scheme handler 与 `image_preview` 模块均为 `#[cfg(target_os = "windows")]`（`lib.rs:17-18,279`；`utils::percent_decode` 同样仅 Windows，`utils/mod.rs:26-27`）。
- tauri 2.11.5 提供 `Manager::asset_protocol_scope() -> scope::fs::Scope`（需 `protocol-asset` feature——`Cargo.toml:20` 已启用），`Scope::allow_directory(path, recursive) -> tauri::Result<()>` 运行时生效。自定义 scheme（image-preview）**不**经过该 scope，必须自行校验。
- `tauri::async_runtime::spawn_blocking` 自由函数存在；`UriSchemeResponder::respond` 接受现有 `Vec<u8>` body 形式。
- 前端 `convertFileSrc` 唯一消费方是 `useThumbnailScheduler`（缩略图），Android 缩略图落盘于 `context.cacheDir/thumb/v2`（`GalleryBridgeV2.kt:58`、`ThumbnailCacheV2.kt:72`）→ 对应 scope 变量 `$APPCACHE`。Windows 预览走 `http://image-preview.localhost/`（`PreviewWindow.tsx:224`）。
- lib 在 **host Linux 不编译**：`platform::get_platform()` 仅有 windows/android 实现（`platform/mod.rs:18-28`），而 `lib.rs:145` 无条件调用。且 `src-tauri/bindings/` 是 gitignored、由依赖 lib 的 `export-bindings` bin 生成 → CI 的 host-linux 无法跑 `cargo test --lib` / gen-types。这决定了 CI 的 job/runner 布局（见任务2 ⚠️ 注记）。
- `futures = "0.3"` 已是依赖（`Cargo.toml:62`）；`filetime`/`tempfile` 已在 dev-dependencies。
- 现有排序键（扫描与插入一致）：`sort_time` 降序，相同则 `modified_time` 降序（`service.rs:164-167` 与 `:280-286`）。

## Tech Stack

Rust 2021 / tauri 2.11.5 / tokio 1.49 / futures 0.3；React 18 + vitest 2（jsdom, globals）+ @testing-library/react；GitHub Actions（actions/checkout@v4、dtolnay/rust-toolchain@stable、Swatinem/rust-cache@v2、oven-sh/setup-bun@v2、android-actions/setup-android@v3、gradle/actions/setup-gradle@v4）。

## Spec（每个任务的问题陈述见 Goal；修复口径如下）

- 任务1：scheme handler 内 canonicalize + save_path 包含校验（越界 403、不存在 404）；线程模型改 `tauri::async_runtime::spawn_blocking`；assetProtocol 静态 scope 收敛为 `$APPCACHE/**`+`$APPDATA/**`，`save_config` 中 save_path 变更后运行时 `allow_directory(new_path, true)`；启用基础 CSP。
- 任务2：4-job workflow：lint（ubuntu）/ test（windows）/ android（ubuntu，legacy debug 变体）/ windows-build（`if: false` TODO）。
- 任务3：两个 hook 返回值 `useMemo`；GalleryCard 注册 effect 用 schedulerRef；VirtualGalleryGrid range 上报按（items 引用 + 范围 key）短路。**不**改 `updateViewport` 的 debounce 语义（理由见任务3末尾）。
- 任务4：扫描改 `buffer_unordered(6)` 并发取 info；`set_files` 改 merge 提交（快照差集保留扫描期间新增），排序键不变。

## Global Constraints

- 本地构建/验证一律 `./build.sh windows android`（含 gen-types、前端构建、双平台测试与编译）；快速循环：Rust `cd src-tauri && cargo.exe test --lib -- <过滤>`（**必须 cargo.exe**），前端 `bun run test -- <路径>`；禁止 `bun`/`npm` build、禁止裸 `cargo`（CI 的 Linux/Windows runner 属例外，见 workflow 内注释）。
- 新源文件必须加 SPDX 头（AGPL-3.0-or-later，格式照抄现有文件头）。本批次仅新增 `.github/workflows/ci.yml`（YAML 注释头，同样写明 SPDX）。
- 遵循既有测试风格：Rust 用 `tempfile::tempdir` + `expect`；前端 hooks 用 `renderHook`/`act` 或 `setupReactRoot` harness。
- 不改变对外行为语义：分页加载、日期跳转、缩略图最终一致性、FTP 上传自动索引管线。
- 每步 TDD：先写失败测试 → 跑到红 → 最小实现 → 跑到绿 → 全量构建 → `git commit`（消息见各步）。

---

## 任务1：image-preview scheme 路径校验 + asset scope 收敛 + 基础 CSP

**Files**

- Modify `src-tauri/src/image_preview/mod.rs`（`use` 区 line 8 改为 `use std::path::{Path, PathBuf};`；在 `content_type_for` 之后新增 `validate_preview_path`；tests 模块末尾追加 4 个测试）
- Modify `src-tauri/src/lib.rs`（line 279-325 重写 scheme handler）
- Modify `src-tauri/src/commands/config.rs`（`save_config`）
- Modify `src-tauri/tauri.conf.json`（`security` 块）

**Interfaces**

- Consumes:
  - `crate::config_service::ConfigService::get_or_default() -> AppConfig`（config_service.rs:68，已核实存在）
  - `tauri::async_runtime::spawn_blocking<F, R>(func: F) -> JoinHandle<R>`
  - `tauri::Manager::asset_protocol_scope(&self) -> tauri::scope::fs::Scope`，`Scope::allow_directory<P: AsRef<Path>>(&self, path: P, recursive: bool) -> tauri::Result<()>`
- Produces:
  - `pub(crate) fn validate_preview_path(requested: &Path, save_root: &Path) -> std::io::Result<Option<PathBuf>>`（`image_preview` 模块内，纯函数可测）
  - `image-preview` 响应语义：路径合法 200 / 越界或非文件 403 / 不存在 404

### 步骤

- [ ] **1.1 写失败测试**：在 `src-tauri/src/image_preview/mod.rs` 的 `mod tests` 末尾追加：

```rust
    #[test]
    fn validate_preview_path_accepts_file_inside_save_root() {
        let dir = std::env::temp_dir().join("cameraftp_test_preview_validate");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("nested")).unwrap();
        let file = dir.join("nested/photo.jpg");
        std::fs::write(&file, b"jpeg-bytes").unwrap();

        let resolved = validate_preview_path(&file, &dir).expect("canonicalize should succeed");
        assert!(resolved.is_some(), "file inside save_root must be accepted");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn validate_preview_path_rejects_escape_via_dotdot() {
        let base = std::env::temp_dir().join("cameraftp_test_preview_escape");
        let _ = std::fs::remove_dir_all(&base);
        let root = base.join("root");
        let outside = base.join("secret.jpg");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(&outside, b"secret").unwrap();

        // 请求 root/../secret.jpg — canonicalize 后位于 root 之外
        let requested = root.join("../secret.jpg");
        let resolved = validate_preview_path(&requested, &root).expect("canonicalize should succeed");
        assert!(resolved.is_none(), "path escaping save_root must be rejected");

        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn validate_preview_path_rejects_directory() {
        let base = std::env::temp_dir().join("cameraftp_test_preview_dir");
        let _ = std::fs::remove_dir_all(&base);
        let sub = base.join("sub");
        std::fs::create_dir_all(&sub).unwrap();

        let resolved =
            validate_preview_path(&sub, &base).expect("canonicalize should succeed");
        assert!(resolved.is_none(), "directory targets must be rejected");

        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn validate_preview_path_errors_for_missing_file() {
        let missing = std::env::temp_dir().join("cameraftp_test_preview_missing/none.jpg");
        let result = validate_preview_path(&missing, std::env::temp_dir());
        assert!(result.is_err(), "missing path should surface an io error");
    }
```

- [ ] **1.2 跑测试确认失败**：`cd src-tauri && cargo.exe test --lib -- validate_preview_path` → 预期编译错误（`cannot find function validate_preview_path`）。
- [ ] **1.3 最小实现**：`image_preview/mod.rs` 顶部 line 8 改 `use std::path::{Path, PathBuf};`，并在 `content_type_for` 之后插入：

```rust
/// 校验 image-preview 请求的路径是否位于保存目录之内。
///
/// 返回:
/// - `Ok(Some(canonical))`: 路径存在、是文件、且位于 `save_root` 之下，
///   返回规范化后的绝对路径（解析 `..`、符号链接与 Windows `\\?\` 前缀）
/// - `Ok(None)`: 路径越界或不是文件，调用方应回 403
/// - `Err(e)`: 路径不存在/无法访问，调用方应回 404
///
/// 两端各自 `canonicalize` 后用 `starts_with` 判断包含关系，可抵御目录穿越。
pub(crate) fn validate_preview_path(
    requested: &Path,
    save_root: &Path,
) -> std::io::Result<Option<PathBuf>> {
    let canonical_requested = requested.canonicalize()?;
    let canonical_root = save_root.canonicalize()?;

    if !canonical_requested.starts_with(&canonical_root) {
        return Ok(None);
    }
    if !canonical_requested.is_file() {
        return Ok(None);
    }
    Ok(Some(canonical_requested))
}
```

- [ ] **1.4 跑测试通过**：`cargo.exe test --lib -- validate_preview_path` → 预期 `test result: ok. 4 passed`。
- [ ] **1.5 重写 handler**（`lib.rs:279-325` 整块替换；`#[cfg(target_os = "windows")]` 门控保持）：

```rust
    #[cfg(target_os = "windows")]
    let builder = builder.register_asynchronous_uri_scheme_protocol(
        "image-preview",
        |ctx, request, responder| {
            use std::path::PathBuf;
            use std::sync::Arc;

            let cache: Arc<ImagePreviewCache> = ctx
                .app_handle()
                .state::<Arc<ImagePreviewCache>>()
                .inner()
                .clone();
            // 每个 preview 请求都必须位于当前配置的 save_path 之下
            let save_root = ctx
                .app_handle()
                .state::<Arc<ConfigService>>()
                .get_or_default()
                .save_path;
            let path_encoded = request
                .uri()
                .path()
                .strip_prefix('/')
                .unwrap_or("")
                .to_string();

            // 每请求一个无上限 OS 线程改为走 tokio blocking 池（天然限流）。
            // responder 满足 Send + 'static，可在 blocking 任务内应答。
            let _ = tauri::async_runtime::spawn_blocking(move || {
                let requested = PathBuf::from(utils::percent_decode(&path_encoded));
                match image_preview::validate_preview_path(&requested, &save_root) {
                    Ok(Some(path)) => {
                        let content_type = image_preview::content_type_for(&path);
                        match cache.get_or_load(&path) {
                            Ok(bytes) => responder.respond(
                                tauri::http::Response::builder()
                                    .status(200)
                                    .header("Content-Type", content_type)
                                    .body(bytes.to_vec())
                                    .unwrap(),
                            ),
                            Err(e) => {
                                tracing::error!(
                                    "Failed to load image preview for {}: {}",
                                    path_encoded,
                                    e
                                );
                                responder.respond(
                                    tauri::http::Response::builder()
                                        .status(500)
                                        .body(b"Failed to load image".to_vec())
                                        .unwrap(),
                                );
                            }
                        }
                    }
                    Ok(None) => {
                        tracing::warn!(
                            requested = %path_encoded,
                            "image-preview request outside save_path rejected"
                        );
                        responder.respond(
                            tauri::http::Response::builder()
                                .status(403)
                                .body(b"Forbidden".to_vec())
                                .unwrap(),
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            requested = %path_encoded,
                            error = %e,
                            "image-preview request path not found"
                        );
                        responder.respond(
                            tauri::http::Response::builder()
                                .status(404)
                                .body(b"Not Found".to_vec())
                                .unwrap(),
                        );
                    }
                }
            });
        },
    );
```

（`ConfigService` 已在 `lib.rs` 导入；`Manager` 已导入。）
- [ ] **1.6 验证编译**：`cd src-tauri && cargo.exe test --lib -- image_preview` → 全部通过。
- [ ] **1.7 全量构建 + 提交**：`./build.sh windows android` → `所有构建完成`。然后：

```
git add -A && git commit -m "feat(security): confine image-preview scheme to configured save_path

- canonicalize + containment check (403 on escape, 404 on missing)
- move per-request std::thread::spawn to bounded spawn_blocking pool"
```

- [ ] **1.8 asset scope 运行时授权**：改 `src-tauri/src/commands/config.rs` 的 `save_config` 为（`mutate_and_persist<F, R>` 泛型返回值，config_service.rs:78 已核实）：

```rust
#[command]
#[instrument(skip(app, config, config_service, file_index))]
pub async fn save_config(
    app: AppHandle,
    config: AppConfig,
    config_service: State<'_, Arc<ConfigService>>,
    file_index: State<'_, Arc<FileIndexService>>,
) -> Result<(), AppError> {
    let old_save_path = config_service.mutate_and_persist(move |current| {
        let old_save_path = current.save_path.clone();
        *current = merge_backend_owned_fields(config, current);
        old_save_path
    })?;
    let new_save_path = config_service.get()?.save_path;

    tracing::info!("Configuration saved successfully");

    if old_save_path != new_save_path {
        tracing::info!("save_path changed from {:?} to {:?}, triggering rescan", old_save_path, new_save_path);
        Arc::clone(&file_index).update_save_path(new_save_path.clone()).await?;
        // 运行时扩展 asset protocol scope，使新保存目录无需重启即可用
        if let Err(e) = app.asset_protocol_scope().allow_directory(&new_save_path, true) {
            tracing::warn!(error = %e, "Failed to extend asset protocol scope for new save_path");
        }
    }

    Ok(())
}
```

（`app: AppHandle` 由 Tauri 注入，前端 `invoke('save_config', { config })` 调用点无需改动。`select_save_directory` 只弹对话框不落盘——真正提交 save_path 的是 `save_config` → `update_save_path`，故授权点放这里。若现签名缺少 `app: AppHandle` 参数则按上方补齐。）
- [ ] **1.9 静态 scope 收敛 + CSP**：改 `src-tauri/tauri.conf.json` 的 `security` 块为：

```json
    "security": {
      "csp": "default-src 'self' ipc: http://ipc.localhost; img-src 'self' asset: http://asset.localhost http://image-preview.localhost data:; style-src 'self' 'unsafe-inline'",
      "assetProtocol": {
        "enable": true,
        "scope": {
          "allow": ["$APPCACHE/**", "$APPDATA/**"],
          "deny": []
        }
      }
    }
```

依据：`$APPCACHE` 覆盖 Android 缩略图（唯一 `convertFileSrc` 消费方），`$APPDATA` 预留；`ipc:`/`http://ipc.localhost` 为 @tauri-apps/api 官方 CSP 示例要求（`node_modules/@tauri-apps/api/core.js:206-207`，Android/Windows 分别对应两种形式）；`style-src 'unsafe-inline'` 因 sonner toast 与 React 行内 `style` 属性；Tauri 会自动为注入脚本追加 nonce/hash，无需手写 script-src；`http://image-preview.localhost` 对应 `PreviewWindow.tsx:224` 的实际 URL 形式。
- [ ] **1.10 配置类改动验证**（无单测路径）：
  - `./build.sh windows android` → 期望构建成功（JSON 合法、构建不回归）。
  - Windows 冒烟（debug 更易看 console）：`./build.sh windows android --debug` 后运行产物：① 主窗口正常、toast/图标/字体正常显示；② FTP 传入照片 → 预览窗口（`http://image-preview.localhost` 图片）正常渲染；③ AI 编辑对话框、日期跳转正常。
  - Android 真机/模拟器冒烟：图库缩略图正常加载（`http://asset.localhost` → `$APPCACHE` 命中）；修改保存目录后图库仍可刷新。
- [ ] **1.11 提交**：

```
git add -A && git commit -m "feat(security): narrow asset protocol scope and enable baseline CSP

- static scope \$APPCACHE/\$APPDATA only; runtime allow_directory on save_path change
- csp: default-src self + ipc origins; asset/image-preview img-src; unsafe-inline styles"
```

**风险注记**：CSP 变更可能破坏 WebView 内 toast/图标/字体加载。1.10 的冒烟清单为必做项；若 Windows 预览窗口图片 403，先查 `save_path` 与实际文件目录是否一致（默认 Pictures 目录）；若 Android 缩略图全灰，检查 `$APPCACHE` 解析（应为 `context.cacheDir`）。

---

## 任务2：最小 CI 流水线（GitHub Actions）

**Files**

- Create `.github/workflows/ci.yml`

**Interfaces**

- Consumes: `scripts/build-raw-alchemy.sh android Debug legacy`、`bunx tauri android build`、`src-tauri/gen/android/gradlew`（已确认存在且可执行）
- Produces: 4 个 job（lint / test / android / windows-build[disabled]）；artifact `debug-legacy-apk`

**⚠️ 与预期不符（实地核查，三处）**：
1. **ubuntu 上不能跑 host-target Rust**：`platform::get_platform()` 仅 windows/android 实现（`platform/mod.rs:18-28`）而 `lib.rs:145` 无条件调用 → `cargo test --lib`/`cargo clippy --lib`（host）在 ubuntu 必失败。对策：clippy 用 `--target aarch64-linux-android`（check/clippy 不链接，无需 NDK）；`cargo test --lib` 放 windows runner（与本地 `cargo.exe` 流程同构）。
2. **ubuntu 上前端 tsc/vitest 缺 bindings**：`src-tauri/bindings/` gitignored（`.gitignore:19`），由依赖 lib 的 `export-bindings` bin 生成 → 只能在 Windows host 上生成。对策：tsc+vitest 并入 windows 的 test job，先 `cargo run --bin export-bindings`。
3. **legacy 变体免 NN 依赖成立**（`build-android.sh:410-417` 设 `CAMERAFTP_NN_DEMOSAIC=0`，`:481-484` `package_nn_android` 仅 neural 调用）→ android job 无需 `fetch-nn-deps.sh`；gradle 单测 task 名 **`testUniversalDebugUnitTest` 成立**（"universal" flavor），且必须在 `tauri android build` 之后跑（settings 文件由 cargo build 生成）。Windows 完整构建确认依赖 standalone LLVM libomp pin（`lib/rawalchemy/CMakeLists.txt:38-59` 无 libomp 即 `FATAL_ERROR`）且 `build-raw-alchemy.sh` windows 路径是 WSL2 专用 → 该 job 置 `if: false` 留 TODO。

### 步骤

- [ ] **2.1 创建 `.github/workflows/ci.yml`**（内容如下，逐行可直接落盘）：

```yaml
# CameraFTP - A Cross-platform FTP companion for camera photo transfer
# Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Minimal CI. NOTE on cargo usage: AGENTS.md forbids bare `cargo` LOCALLY
# (WSL host cannot build Windows artifacts). On GitHub runners the runner OS
# IS the target (linux->android cross for lint, windows host for tests), so
# plain cargo is correct here and does not conflict with the local rule.
name: CI

on:
  push:
    branches: [main]
  pull_request:

concurrency:
  group: ci-${{ github.ref }}
  cancel-in-progress: true

env:
  CARGO_TERM_COLOR: always

jobs:
  lint:
    runs-on: ubuntu-latest
    timeout-minutes: 20
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt
      - name: Install shellcheck
        run: sudo apt-get update && sudo apt-get install -y shellcheck
      - uses: Swatinem/rust-cache@v2
        with:
          workspaces: src-tauri
      - name: rustfmt
        run: cargo fmt --all --check
        working-directory: src-tauri
      - name: Add Android target
        run: rustup target add aarch64-linux-android
      # The lib does not compile for host Linux (platform::get_platform only
      # has windows/android impls), so clippy runs against the Android target.
      # clippy does not link -> no NDK required.
      - name: clippy (android target)
        run: cargo clippy --lib --target aarch64-linux-android -- -D warnings
        working-directory: src-tauri
      - name: shellcheck
        run: shellcheck build.sh scripts/*.sh

  test:
    # Windows host: the lib only compiles for windows/android cfg paths, and
    # tsc needs the gitignored src-tauri/bindings generated by the
    # export-bindings bin (which depends on the lib) — same environment as
    # the local cargo.exe flow.
    runs-on: windows-latest
    timeout-minutes: 60
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
        with:
          workspaces: src-tauri
      - uses: oven-sh/setup-bun@v2
        with:
          bun-version: 1.4.0
      - name: Stub frontend dist for generate_context!
        shell: bash
        run: mkdir -p dist
      - name: Generate TS bindings
        run: cargo run --quiet --locked --bin export-bindings
        working-directory: src-tauri
      - name: Rust tests
        run: cargo test --lib --locked
        working-directory: src-tauri
      - name: Install frontend deps
        run: bun install --frozen-lockfile
      - name: Type check
        run: bunx tsc --noEmit
      - name: Frontend tests
        run: bun run test

  android:
    runs-on: ubuntu-latest
    timeout-minutes: 90
    steps:
      - uses: actions/checkout@v4
        with:
          submodules: recursive
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: aarch64-linux-android
      - uses: actions/setup-java@v4
        with:
          distribution: temurin
          java-version: 21
      - uses: android-actions/setup-android@v3
      - uses: gradle/actions/setup-gradle@v4
      - uses: oven-sh/setup-bun@v2
        with:
          bun-version: 1.4.0
      - name: Install NDK + platform
        run: |
          sdkmanager --install "ndk;28.2.13676358" "platforms;android-36" "platform-tools"
          echo "NDK_HOME=$ANDROID_HOME/ndk/28.2.13676358" >> "$GITHUB_ENV"
      - uses: Swatinem/rust-cache@v2
        with:
          workspaces: src-tauri
      - name: Install frontend deps
        run: bun install --frozen-lockfile
      # legacy variant only: CAMERAFTP_NN_DEMOSAIC=0 -> no ORT/QNN/DirectML
      # fetch needed (see scripts/build-android.sh build_android()).
      - name: Build RawAlchemyCpp (legacy)
        run: ./scripts/build-raw-alchemy.sh android Debug legacy
      - name: Stage native libs
        run: |
          jni_dir="src-tauri/gen/android/app/extra-jniLibs/arm64-v8a"
          mkdir -p "$jni_dir"
          rm -f "$jni_dir"/*.so
          cp src-tauri/lib/rawalchemy/build-android-arm64/libraw_alchemy.so "$jni_dir/libraw_alchemy_core.so"
          omp="$(find "$NDK_HOME/toolchains/llvm/prebuilt" -path "*/aarch64/libomp.so" | head -1)"
          cp "$omp" "$jni_dir/libomp.so"
      - name: Build debug APK (legacy)
        run: bunx tauri android build --debug --apk --target aarch64
        env:
          CAMERAFTP_NN_DEMOSAIC: 0
      # Must run AFTER the tauri build: tauri.settings.gradle and
      # app/tauri.build.gradle.kts are generated by the cargo build
      # (see comment in scripts/build-android.sh main()).
      - name: Android unit tests
        run: ./gradlew testUniversalDebugUnitTest
        working-directory: src-tauri/gen/android
      - uses: actions/upload-artifact@v4
        with:
          name: debug-legacy-apk
          path: src-tauri/gen/android/app/build/outputs/apk/universal/debug/*.apk

  windows-build:
    # TODO: full Windows build needs the standalone LLVM/clang-cl FFI pipeline
    # (lib/rawalchemy/CMakeLists.txt pins OpenMP to standalone LLVM libomp and
    # FATAL_ERRORs without it) and scripts/build-raw-alchemy.sh's windows path
    # is WSL2-only (wslpath/cmd.exe bridges). Revisit by porting
    # build_windows.bat to a windows runner + `choco install llvm ninja`.
    if: false
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
        with:
          submodules: recursive
```

- [ ] **2.2 本地静态自检**：`python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/ci.yml'))" && echo YAML_OK` → 期望 `YAML_OK`。
- [ ] **2.3 真实运行验证**：推送分支并开 PR（触发 `pull_request`）。期望：`lint`、`test`、`android` 三个 job 全绿，`windows-build` 显示 skipped。首次运行预计 lint ~8min、test ~20min、android ~45min（rust-cache/gradle 缓存命中后显著下降）。若 clippy 报存量 warning，在**同一 PR** 内按提示修复源码后重推（`-D warnings` 语义保留）。
- [ ] **2.4 已知风险处理**：若 NDK `28.2.13676358` 在 sdkmanager 不可用，任选 r27+ 版本并同步两处字符串；若子模块 `RawAlchemyCpp` 仓库为私有需在 checkout 配 `token`（当前公开仓库 `github.com/GoldJohnKing/RawAlchemyCpp`，默认 GITHUB_TOKEN 可拉取）。
- [ ] **2.5 提交**：

```
git add .github/workflows/ci.yml && git commit -m "ci: add minimal GitHub Actions pipeline (lint / test / android)

- lint: fmt + clippy(android target, host-linux lib not compilable) + shellcheck
- test(windows): gen-types + cargo test --lib + tsc + vitest
- android: legacy debug APK + testUniversalDebugUnitTest (post-tauri-build)
- windows-build: disabled with TODO (standalone LLVM libomp pin, WSL-only scripts)"
```

---

## 任务3：图库重渲染链稳定化

**Files**

- Modify `src/hooks/useGalleryPager.ts`（import 行；返回块）
- Modify `src/hooks/useThumbnailScheduler.ts`（import 行；返回块）
- Modify `src/components/GalleryCard.tsx`（line 38 后新增 ref；line 91-95 effect 重写）
- Modify `src/components/VirtualGalleryGrid.tsx`（line 94 后新增 ref；line 210-237 effect 重写）
- Modify `src/hooks/__tests__/useGalleryPager.test.tsx`（追加 1 用例）
- Modify `src/hooks/__tests__/useThumbnailScheduler.test.ts`（追加 1 用例）
- Modify `src/components/__tests__/VirtualGalleryGrid.test.tsx`（追加 1 用例；`resizeMock.triggerResize` 基建已核实存在，:58 起 12 处使用）

**Interfaces**

- Consumes/Produces：两个 hook 的对外签名**不变**；`VirtualGalleryGridProps` 不变。唯一语义变化：range 上报在"items 引用与范围 key 均未变"时跳过回调。

### 步骤

- [ ] **3.1 写失败测试（引用稳定性 ×2 + range 短路 ×1）**：

`useThumbnailScheduler.test.ts` 顶层 describe 内追加：

```ts
  it('keeps the returned object identity stable across rerenders', () => {
    const { result, rerender } = renderHook(() =>
      useThumbnailScheduler({ debounceMs: TEST_DEBOUNCE }),
    );

    const first = result.current;
    rerender();

    expect(result.current).toBe(first);
  });
```

（若该文件现有用例未以 options 参数调用 hook，则去掉 `{ debounceMs: TEST_DEBOUNCE }` 参数以对齐现有调用形式。）

`useGalleryPager.test.tsx` describe 内追加：

```tsx
  it('returns a stable object reference across rerenders without new data', async () => {
    listMediaPageMock.mockResolvedValueOnce(
      makePage([makeItem('media-1')], null, 'rev-1'),
    );

    await renderHarness();
    await clickLoadNext(getContainer);

    const first = latestResult!;

    await act(async () => {
      getRoot().render(<PagerHarness />);
      await flush();
    });

    expect(latestResult).toBe(first);
  });
```

（`latestResult`/`PagerHarness`/`renderHarness`/`clickLoadNext`/`makePage`/`makeItem`/`flush` 为该测试文件既有 harness 素材；若变量名略有出入，以文件内既有命名为准对齐。）

`VirtualGalleryGrid.test.tsx` describe 内追加：

```tsx
  it('does not re-report the range when only callback identities change', async () => {
    const items = makeItems(90); // 30 rows — same array instance for both renders
    const onRangeChange1 = vi.fn();

    await act(async () => {
      getRoot().render(
        <VirtualGalleryGrid
          items={items}
          thumbnails={new Map()}
          loadingThumbs={new Set()}
          onItemClick={vi.fn()}
          onRangeChange={onRangeChange1}
        />
      );
      await flush();
    });

    const gridContainer = getContainer().querySelector('[data-testid="virtual-grid-container"]');
    expect(gridContainer).toBeTruthy();
    if (gridContainer) {
      act(() => {
        resizeMock.triggerResize(gridContainer, 360);
      });
    }
    await flush();

    const callsAfterMount = onRangeChange1.mock.calls.length;
    expect(callsAfterMount).toBeGreaterThanOrEqual(1);

    // Same items/scroll, brand-new callback identities — exactly what happens
    // on every GalleryCard render while thumbnails stream in.
    const onRangeChange2 = vi.fn();
    await act(async () => {
      getRoot().render(
        <VirtualGalleryGrid
          items={items}
          thumbnails={new Map()}
          loadingThumbs={new Set()}
          onItemClick={vi.fn()}
          onRangeChange={onRangeChange2}
          onNearEnd={vi.fn()}
        />
      );
      await flush();
    });

    expect(onRangeChange2).not.toHaveBeenCalled();
    expect(onRangeChange1.mock.calls.length).toBe(callsAfterMount);
  });
```

- [ ] **3.2 跑测试确认失败**：三个测试文件分别 `bun run test -- <路径>` → 各自新用例 fail。
- [ ] **3.3 hook 返回值 memo 化**：

`useGalleryPager.ts` import 改为 `import { useCallback, useMemo, useRef, useState } from 'react';`，返回块替换为：

```ts
  return useMemo(
    () => ({
      items,
      cursor,
      totalCount,
      isLoading,
      error,
      loadNextPage,
      reload,
      loadAll,
      removeItems,
      addItems,
    }),
    [items, cursor, totalCount, isLoading, error, loadNextPage, reload, loadAll, removeItems, addItems],
  );
```

（以文件实际返回的字段名为准——执行时先读现有返回块，将其中全部字段与对应变量逐一填入对象与依赖数组，不增不减。）

`useThumbnailScheduler.ts` import 改为 `import { useCallback, useEffect, useMemo, useRef, useState } from 'react';`，返回块替换为：

```ts
  return useMemo(
    () => ({
      thumbnails,
      loadingThumbs,
      updateViewport,
      removeThumbs,
      cleanup,
      registerMedia,
    }),
    [thumbnails, loadingThumbs, updateViewport, removeThumbs, cleanup, registerMedia],
  );
```

（同上，以现有返回块字段为准逐一对应。）

- [ ] **3.4 跑 hooks 测试通过**：`bun run test -- src/hooks/__tests__/useGalleryPager.test.tsx src/hooks/__tests__/useThumbnailScheduler.test.ts` → 全绿。
- [ ] **3.5 提交**：

```
git add -A && git commit -m "perf(gallery): memoize hook return objects

useGalleryPager/useThumbnailScheduler returned fresh object literals every
render, cascading identity churn into every dependent effect/callback."
```

- [ ] **3.6 GalleryCard 注册 effect 去 scheduler 依赖**（`useRef` 已导入）：`const scheduler = useThumbnailScheduler();` 之后新增，并替换 line 91-95 effect：

```tsx
  // scheduler 对象随 thumbnails 变化而变化（见 useThumbnailScheduler 的
  // useMemo 依赖），用 ref 持有以避免 items 未变时重跑 O(n) 注册
  const schedulerRef = useRef(scheduler);
  schedulerRef.current = scheduler;

  // Register media metadata with scheduler when items change
  useEffect(() => {
    if (pager.items.length > 0) {
      schedulerRef.current.registerMedia(pager.items);
    }
  }, [pager.items]);
```

- [ ] **3.7 VirtualGalleryGrid range 上报短路**：`lastArmedHighlightRef` 之后新增：

```tsx
  // 上次 range 上报的（items 引用 + 范围 key）。回调引用（onRangeChange/
  // onNearEnd）在父组件每次渲染都可能变化，但 items 与可见范围未变时跳过
  // 上报 — 否则缩略图批量到达期间每次渲染都重置 scheduler 的 60ms debounce。
  const lastReportedRangeRef = useRef<{ items: MediaItemDto[]; key: string } | null>(null);
```

line 210-237 effect 替换为：

```tsx
  // Report range changes and trigger infinite scroll
  useEffect(() => {
    if (!onRangeChange) return;
    if (items.length === 0) return;
    // Skip if container height is not yet measured - prevents incorrect range calculation
    if (containerHeight === 0) return;

    const visibleStartIdx = visibleStartRow * COLUMNS;
    const visibleEndIdx = Math.min(items.length, (visibleEndRow + 1) * COLUMNS);
    const visibleIds = items.slice(visibleStartIdx, visibleEndIdx).map((item) => item.mediaId);

    // 索引范围 + 首尾 mediaId 构成 key：范围相同但数据变了（删除/刷新/
    // 追加换页）仍需上报；items 数组引用同时参与比较，保证 reload 后同
    // 位置也会重新上报（触发缩略图重新请求）。
    const rangeKey = `${visibleStartIdx}:${visibleEndIdx}:${visibleIds[0] ?? ''}:${
      visibleIds[visibleIds.length - 1] ?? ''
    }`;
    const last = lastReportedRangeRef.current;
    if (last && last.items === items && last.key === rangeKey) return;
    lastReportedRangeRef.current = { items, key: rangeKey };

    const nearbyStartIdx = startRow * COLUMNS;
    const nearbyEndIdx = Math.min(items.length, (endRow + 1) * COLUMNS);
    const nearbyIds = items
      .slice(nearbyStartIdx, nearbyEndIdx)
      .map((item) => item.mediaId)
      .filter((id) => !visibleIds.includes(id));

    onRangeChange(visibleIds, nearbyIds);

    // Trigger infinite scroll when near the end
    if (onNearEnd && totalRows > 0) {
      const rowsRemaining = totalRows - visibleEndRow - 1;
      if (rowsRemaining <= NEAR_END_THRESHOLD) {
        onNearEnd();
      }
    }
  }, [items, visibleStartRow, visibleEndRow, startRow, endRow, onRangeChange, onNearEnd, containerHeight, totalRows]);
```

- [ ] **3.8 跑全部相关测试**：`bun run test -- src/components/__tests__/VirtualGalleryGrid.test.tsx src/components/__tests__/GalleryCard.virtualized.test.tsx src/components/__tests__/GalleryCard.date-jump.test.tsx src/components/__tests__/GalleryCard.filter.test.tsx` → 全绿（含既有 `reports visible range changes on scroll`——scroll 改变 key 仍会上报）。
- [ ] **3.9 全量回归**：`./build.sh windows android`（现有 hooks 测试必须全绿）。
- [ ] **3.10 提交**：

```
git add -A && git commit -m "perf(gallery): stop redundant range reports and scheduler re-registration

- GalleryCard: hold scheduler via ref; registration effect depends on items only
- VirtualGalleryGrid: skip onRangeChange/onNearEnd when items reference and
  visible-range key are unchanged (thumbnail-arrival render churn)"
```

**④（leading+trailing debounce）不实施——理由**：③ 的短路已在源头消除了"结果流突发期间反复重置 debounce"的根因（同 range 的重复 `updateViewport` 不再发生）；滚动本身重置 debounce 是 trailing 防抖的预期语义。引入 leading 派发会在 cancels 尚未计算时先派发请求、扩大取消/重派发的窗口，且改变现有测试断言的"仅一次 enqueue"行为，收益为零、回归面非零。若实测仍有延迟，再单独立项。

---

## 任务4：索引扫描并发化 + 合并提交

**Files**

- Modify `src-tauri/src/file_index/service.rs`（imports；新增 `const SCAN_CONCURRENCY`；`scan_directory` + `scan_directory_iterative` 重写；新增 `merge_scan_result`；tests 模块追加 4 个测试）

**Interfaces**

- Consumes: `futures::stream::iter(...).map(...).buffer_unordered(n)`（`futures = "0.3"` 已在依赖）；`FileIndex.path_set`（service 内可访问）；`set_files`、`emit_file_index_changed`、`get_file_info`（既有）
- Produces（均为 crate 私有）:
  - `const SCAN_CONCURRENCY: usize = 6;`
  - `async fn collect_image_paths(&self, root: &Path) -> Result<Vec<PathBuf>, AppError>`（私有）
  - `fn merge_scan_result(scanned: Vec<FileInfo>, existing: &[FileInfo], pre_scan_paths: &std::collections::HashSet<PathBuf>) -> Vec<FileInfo>`（关联函数，纯函数可测；排序键与现状一致）

### 步骤

- [ ] **4.1 写失败测试**：`service.rs` tests 模块（确认 `use std::path::{Path, PathBuf};` 已有，补 `use std::collections::HashSet;`），在末尾追加：

```rust
    // ---- 并发扫描与合并提交 ----

    #[test]
    fn merge_scan_result_preserves_files_added_during_scan() {
        let scanned = vec![make_file_info("/images/a.jpg", 3000)];
        let existing = vec![
            make_file_info("/images/a.jpg", 3000),
            // FTP Put 通道在 readdir 之后、提交之前插入
            make_file_info("/images/new_from_ftp.jpg", 5000),
        ];
        let pre_scan_paths: HashSet<PathBuf> =
            [PathBuf::from("/images/a.jpg")].into_iter().collect();

        let merged = FileIndexService::merge_scan_result(scanned, &existing, &pre_scan_paths);

        assert_eq!(merged.len(), 2);
        // 排序：sort_time 降序 → new_from_ftp(5000) 在前
        assert_eq!(merged[0].filename, "new_from_ftp.jpg");
    }

    #[test]
    fn merge_scan_result_drops_files_deleted_during_scan() {
        let scanned = vec![make_file_info("/images/a.jpg", 3000)];
        let existing = vec![
            make_file_info("/images/a.jpg", 3000),
            make_file_info("/images/deleted.jpg", 1000),
        ];
        let pre_scan_paths: HashSet<PathBuf> =
            existing.iter().map(|f| f.path.clone()).collect();

        let merged = FileIndexService::merge_scan_result(scanned, &existing, &pre_scan_paths);

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].filename, "a.jpg");
    }

    #[test]
    fn merge_scan_result_prefers_scanned_metadata_for_same_path() {
        let scanned = vec![make_file_info("/images/a.jpg", 9000)];
        let existing = vec![make_file_info("/images/a.jpg", 3000)];
        let pre_scan_paths: HashSet<PathBuf> =
            [PathBuf::from("/images/a.jpg")].into_iter().collect();

        let merged = FileIndexService::merge_scan_result(scanned, &existing, &pre_scan_paths);

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].sort_time, 9000);
    }

    #[tokio::test]
    async fn scan_directory_concurrent_finds_all_images_in_nested_dirs() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let save_path = temp_dir.path().join("images");
        std::fs::create_dir_all(save_path.join("sub_a/sub_b")).expect("create nested dirs");

        let mut expected = std::collections::HashSet::new();
        for i in 0..12 {
            let name = format!("img_{:02}.jpg", i);
            std::fs::write(save_path.join("sub_a").join(&name), b"jpeg").expect("write file");
            expected.insert(name);
        }
        for i in 12..20 {
            let name = format!("img_{:02}.jpg", i);
            std::fs::write(save_path.join("sub_a/sub_b").join(&name), b"jpeg").expect("write file");
            expected.insert(name);
        }
        std::fs::write(save_path.join("notes.txt"), b"not an image").expect("write file");

        let config_service = ConfigService::new_with_path(temp_dir.path().join("config.json"));
        config_service
            .mutate_and_persist(|config| {
                config.save_path = save_path.clone();
            })
            .expect("persist config");

        let service = FileIndexService::new(Arc::new(config_service));
        service.scan_directory().await.expect("scan");

        let files = service.get_files().await;
        assert_eq!(files.len(), 20, "all supported images must be indexed");
        let names: std::collections::HashSet<String> =
            files.iter().map(|f| f.filename.clone()).collect();
        assert_eq!(names, expected);
    }
```

- [ ] **4.2 跑测试确认失败**：`cd src-tauri && cargo.exe test --lib -- merge_scan_result` → 编译错误（`no function named merge_scan_result`）；`scan_directory_concurrent` 同样编译失败，符合红灯预期。
- [ ] **4.3 实现**：

顶部 imports 追加 `use std::collections::HashSet;`，模块常量区新增：

```rust
/// 扫描期 get_file_info 并发度。EXIF 解析在 tokio spawn_blocking 池上执行，
/// 池大小天然限流；此值只限制同时在飞的 future 数，避免瞬时占满阻塞池。
const SCAN_CONCURRENCY: usize = 6;
```

将 `scan_directory` 与 `scan_directory_iterative`（保留其目录遍历语义为 `collect_image_paths`）整体替换为：

```rust
    /// 扫描目录建立索引
    pub async fn scan_directory(&self) -> Result<(), AppError> {
        let save_path = self.save_path.read().await.clone();
        info!("Starting directory scan: {:?}", save_path);

        // 提交前快照：识别"扫描期间通过 FTP Put 通道新进入索引"的文件，
        // 提交时保留它们（见 merge_scan_result）
        let pre_scan_paths: HashSet<PathBuf> = {
            let index = self.index.read().await;
            index.path_set.clone()
        };

        let paths = self.collect_image_paths(&save_path).await?;

        // 并发获取文件信息（EXIF 解析经 spawn_blocking，见 read_exif_time）
        let infos = {
            futures::stream::iter(paths)
                .map(|path| async move {
                    let metadata = tokio::fs::metadata(&path).await
                        .map_err(|e| AppError::Other(format!("Failed to get metadata: {}", e)))?;
                    self.get_file_info(&path, &metadata).await
                })
                .buffer_unordered(SCAN_CONCURRENCY)
                .collect::<Vec<Result<FileInfo, AppError>>>()
                .await
        };

        let mut files: Vec<FileInfo> = infos
            .into_iter()
            .filter_map(|r| match r {
                Ok(file_info) => Some(file_info),
                Err(e) => {
                    warn!("Failed to get file info during scan: {}", e);
                    None
                }
            })
            .collect();

        // 按 sort_time 降序，相同则按 modified_time 降序（新文件优先）
        files.sort_by(|a, b| {
            b.sort_time.cmp(&a.sort_time)
                .then_with(|| b.modified_time.cmp(&a.modified_time))
        });

        let mut index = self.index.write().await;
        let existing: Vec<FileInfo> = index.files().as_ref().clone();
        let merged = Self::merge_scan_result(files, &existing, &pre_scan_paths);
        index.current_index = merged.first().map(|_| 0);
        let count = merged.len();
        index.set_files(merged);

        info!("Directory scan complete: {} files found", count);

        drop(index);
        self.emit_file_index_changed().await;

        Ok(())
    }

    /// 迭代遍历目录（工作栈），收集受支持的图片路径
    async fn collect_image_paths(&self, root: &Path) -> Result<Vec<PathBuf>, AppError> {
        let mut dirs_to_process = vec![root.to_path_buf()];
        let mut paths = Vec::new();

        while let Some(dir) = dirs_to_process.pop() {
            let mut entries = tokio::fs::read_dir(&dir).await
                .map_err(|e| AppError::Other(format!("Failed to read dir: {}", e)))?;

            while let Some(entry) = entries.next_entry().await
                .map_err(|e| AppError::Other(format!("Failed to read entry: {}", e)))?
            {
                let path = entry.path();
                let metadata = match entry.metadata().await {
                    Ok(m) => m,
                    Err(_) => continue,
                };

                if metadata.is_dir() {
                    dirs_to_process.push(path);
                } else if metadata.is_file() {
                    if crate::image_utils::is_supported_image(&path) {
                        paths.push(path);
                    }
                }
            }
        }

        Ok(paths)
    }

    /// 合并扫描结果与索引中"扫描期间新增"的条目。
    ///
    /// FTP Put 监听（ftp/listeners.rs）独立 spawn 调 add_file，可在扫描的
    /// readdir 与 set_files 提交之间插入。保留规则：
    /// - existing 中路径不在 scanned 结果、也不在 pre_scan_paths 快照中
    ///   → 扫描开始后才进入索引的新文件，保留；
    /// - 在 pre_scan_paths 中但不在 scanned 中 → 扫描期间已删除，丢弃
    ///   （与旧 set_files 整体覆盖行为一致）；
    /// - 同一路径以 scanned 的新元数据为准。
    fn merge_scan_result(
        scanned: Vec<FileInfo>,
        existing: &[FileInfo],
        pre_scan_paths: &HashSet<PathBuf>,
    ) -> Vec<FileInfo> {
        let scanned_paths: HashSet<&PathBuf> = scanned.iter().map(|f| &f.path).collect();

        let mut merged = scanned;
        merged.extend(
            existing
                .iter()
                .filter(|f| !scanned_paths.contains(&f.path) && !pre_scan_paths.contains(&f.path))
                .cloned(),
        );

        merged.sort_by(|a, b| {
            b.sort_time.cmp(&a.sort_time)
                .then_with(|| b.modified_time.cmp(&a.modified_time))
        });
        merged
    }
```

（`self.save_path`/`index.files()`/`index.set_files`/`index.current_index` 等字段与访问器的实际形态以 service.rs 现状为准——执行时先读 `scan_directory` 现有实现，遍历/排序/提交语义保持不变，仅替换并发取 info 与 merge 提交两处。）

- [ ] **4.4 跑测试通过**：`cargo.exe test --lib -- merge_scan_result` → `3 passed`；`cargo.exe test --lib -- scan_directory_concurrent` → `1 passed`。
- [ ] **4.5 全量回归（含全部既有 file_index 测试）**：`cargo.exe test --lib -- file_index` → 既有 + 新 4 项全绿；随后 `./build.sh windows android` → 构建成功。
- [ ] **4.6 提交（两个逻辑单元）**：

```
git add -A && git commit -m "perf(file-index): fetch file infos concurrently during directory scan

buffer_unordered(6) over collect_image_paths; spawn_blocking pool bounds
actual EXIF parse parallelism. Sort keys unchanged (sort_time desc, mtime desc)."
```

```
git add -A && git commit -m "fix(file-index): merge-commit scan results to keep files added during scan

snapshot path_set before scan; preserve index entries added by the FTP Put
listener between readdir and commit (previously lost to wholesale set_files)"
```

**已知边界（如实记录，不在本批处理）**：扫描期间发生的 `remove_file`（FTP Deleted）若发生在 readdir 之后，scanned 结果仍含该文件，合并会短暂"复活"它——与改动前 `set_files` 整体覆盖的行为一致，由后续 watcher/put 事件或下次扫描收敛；`update_save_path` 的 stop-first 流程保证 watcher 通道无此竞态。

---

## 完成定义（批次级）

- [ ] `./build.sh windows android` 全绿（含 Rust `--lib` 测试、前端全部 vitest、双平台编译）。
- [ ] Windows + Android 冒烟通过（任务1 的 1.10 清单）。
- [ ] CI 在 PR 上三个活跃 job 全绿、`windows-build` 按预期 skipped。
- [ ] `git log --oneline` 显示本计划全部 commit 消息，无合并残余。
