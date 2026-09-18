# 批次 3：护栏与体验 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. 执行前先读 `AGENTS.md`（仓库根目录）；禁止跳过"确认失败"步骤；每个任务独立 commit。

**Goal**: 在不改变功能行为的前提下收紧安全面（FileProvider 暴露面、签名口令、NN 依赖供应链）、升级前端工具链（Vite 5→6）、建立版本同步护栏，并消除 `ConfigService` 写路径持锁 fsync 与读路径整份 `clone` 两个热路径缺陷。

**Architecture**: Tauri v2 单体应用：React 18 + TS + Zustand 前端（Vite 构建、vitest 测试）→ Rust 后端（`ConfigService` 全局单例经 `OnceLock` 分发，`commands/*` 为 IPC 边界）；Android 端为 `src-tauri/gen/android`（生成但已深度手工定制）+ `scripts/build-android.sh`。

**Tech Stack**: Rust 2021（tauri 2 / tokio / std::sync）、TypeScript（Vite/vitest/jsdom）、Bun（NJU npm 镜像）、Gradle 8.14 / Kotlin / Robolectric 4.16、bash（`build.sh` 统一入口）。

**Spec**: 7 个任务均经对抗性审查确认成立；任务内代码以当前 `HEAD` 代码库为准（本计划逐文件核对过行号与内容，发起方已复核关键接口与语义）。发现与原假设冲突处已标注 ⚠️。

## 跨批次衔接注记（必读）

本批次任务 6 与前两批存在两处交叠，执行顺序不同则编辑点不同：

1. **`save_auth_config`（批次1任务3 ↔ 批次3任务6步骤5）**：批次1任务3 已把该命令改为 `async fn` 并用 `spawn_blocking` 包裹同步的 `save_auth_config_with_service`。若批次1已执行，本批任务6步骤5 对该命令的处理变为：**删除 spawn_blocking 包裹层**，改为直接 `save_auth_config_with_service(...).await`（helper 已异步化，`mutate_and_persist_async` 内部自行落盘到 blocking 池；Argon2 哈希仍保留在 helper 内的 `spawn_blocking` 中——见步骤5代码）。若批次1未执行，按步骤5从同步版本直接改造，Argon2 同样包进 helper 内的 `spawn_blocking`（两种路径最终状态一致）。
2. **`save_config`（批次2任务1.8 ↔ 批次3任务6步骤5）**：批次2任务1.8 重写过 `save_config`（闭包返回 `old_save_path`）。本批任务6仅把其中的 `mutate_and_persist(...)?` 换成 `mutate_and_persist_async(...).await?`，**闭包体保持原样**，其余逻辑（`allow_directory` 等）不动。

## Global Constraints

1. **构建入口**：任何代码改动验证 = `./build.sh windows android`（含前端 tsc+vite build、Rust 测试、Android gradlew test）。禁止旁路构建。
2. **快速循环**：Rust 用 `cargo.exe test --lib -- <过滤>`（WSL 内必须 `cargo.exe`）；前端用 `bun run test -- <路径>`；Android Kotlin 测试经 `./build.sh android`（`gradlew test` 在构建后段）。
3. **新源文件必须加 SPDX 头**（AGPL-3.0-or-later；TS/Kotlin 用 `/** */` 块注释，shell 用 `#`）。
4. **禁止 LSP 工具**（`lsp_*` 全部会挂起）。
5. `src-tauri/gen/android/` 是生成但已手工定制的目录，可直接编辑；但 `gen/android/app/build/` 下的同名文件是构建产物，**不要**编辑。`.gitignore:72` 忽略了 `gen/.../generated/`，全库 grep 该目录需 `rg --no-ignore`。
6. `bunfig.toml` 配置了 NJU 镜像，`bun add` 自动走镜像。
7. 复选框步骤按顺序执行：**写失败测试 → 确认失败 → 最小实现 → 确认通过 → `./build.sh windows android` → commit**。配置类改动用"验证命令 + 期望输出"替代失败测试。
8. commit 信息格式：`<scope>: <imperative summary>`。

---

### Task 1: FileProvider 收窄（Android）

### Files
- `src-tauri/gen/android/app/src/main/res/xml/file_paths.xml:2-20`（`:4` `<external-path name="external_files" path="." />`、`:7` `pictures`、`:10` `dcim` 三个共享存储根）
- `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/generated/RustWebChromeClient.kt:468-485`（唯一消费方：`:471` `FileProvider.getUriForFile(activity, packageName + ".fileprovider", photoFile)`；`:483` `activity.getExternalFilesDir(Environment.DIRECTORY_PICTURES)`）
- `src-tauri/gen/android/app/src/main/AndroidManifest.xml:102-111`（provider 声明，不改）
- 新建 `src-tauri/gen/android/app/src/test/java/com/gjk/cameraftpcompanion/FileProviderPathsTest.kt`

### Interfaces
- **Consumes**: AndroidX `FileProvider` 路径根映射：`<external-path>` → 共享存储根（过度暴露）；`<external-files-path path="Pictures">` → `Context.getExternalFilesDir(null)/Pictures` = `Android/data/<pkg>/files/Pictures`，与 `RustWebChromeClient.createImageFile` 的临时文件目录精确匹配。
- **Produces**: 收窄后的 `@xml/file_paths`；Robolectric 行为测试（不加 `manifest = Config.NONE`——需要真实 manifest 注册的 provider）。

背景核查（已核实）：全库唯一 `getUriForFile` 消费方是 `RustWebChromeClient.kt:471`（相机拍照临时文件）；分享走 MediaStore content URI 不经 FileProvider。`files-path`/`cache-path`/`external-cache-path` 为应用私有目录，保留。

### 步骤
- [ ] 1. 新建 `FileProviderPathsTest.kt`：

```kotlin
/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

package com.gjk.cameraftpcompanion

import android.content.Context
import android.os.Environment
import androidx.core.content.FileProvider
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.fail
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import java.io.File

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [35])
class FileProviderPathsTest {

    private val authority = "com.gjk.cameraftpcompanion.fileprovider"

    private fun context(): Context = ApplicationProvider.getApplicationContext()

    @Test
    fun camera_capture_temp_file_is_shareable() {
        // RustWebChromeClient.createImageFile builds its capture temp file in
        // getExternalFilesDir(DIRECTORY_PICTURES) — the FileProvider must
        // resolve exactly that root.
        val dir = context().getExternalFilesDir(Environment.DIRECTORY_PICTURES)!!
        dir.mkdirs()
        val photo = File(dir, "JPEG_20260916_000000.jpg")
        photo.writeBytes(byteArrayOf(0xFF.toByte()))

        val uri = FileProvider.getUriForFile(context(), authority, photo)

        assertNotNull(uri)
        assertEquals("content", uri.scheme)
    }

    @Test
    fun shared_storage_roots_are_not_exposed() {
        // The over-broad <external-path> roots must be gone: a file on shared
        // storage must NOT be convertible into a grantable content URI.
        val outsider = File(Environment.getExternalStorageDirectory(), "DCIM/leak.jpg")
        try {
            FileProvider.getUriForFile(context(), authority, outsider)
            fail("external storage root must not be shareable via FileProvider")
        } catch (expected: IllegalArgumentException) {
            // "Failed to find configured root" — expected after narrowing
        }
    }
}
```

- [ ] 2. 确认失败（red）：`./build.sh android`。期望 `shared_storage_roots_are_not_exposed` **FAILED**（当前 `<external-path path="." />` 匹配共享存储，`fail()` 被触达），`camera_capture_temp_file_is_shareable` PASSED。
  - 若 Robolectric 无法从合并 manifest 实例化 provider（报未注册），退化方案（保持 red 属性）：

```kotlin
    @Test
    fun file_paths_declares_no_shared_storage_roots() {
        val parser = context().resources.getXml(R.xml.file_paths)
        val roots = mutableListOf<Pair<String, String>>()
        while (parser.next() != org.xmlpull.v1.XmlPullParser.END_DOCUMENT) {
            if (parser.eventType == org.xmlpull.v1.XmlPullParser.START_TAG) {
                roots += parser.name to (parser.getAttributeValue(null, "path") ?: "")
            }
        }
        org.junit.Assert.assertFalse(roots.contains("external-path" to "."))
        org.junit.Assert.assertTrue(roots.contains("external-files-path" to "Pictures"))
    }
```

- [ ] 3. 最小实现：`file_paths.xml` 全文件替换为：

```xml
<?xml version="1.0" encoding="utf-8"?>
<paths xmlns:android="http://schemas.android.com/apk/res/android">
    <!-- Camera capture temp files (RustWebChromeClient.createImageFile):
         getExternalFilesDir(DIRECTORY_PICTURES) = Android/data/<pkg>/files/Pictures -->
    <external-files-path name="external_files" path="Pictures" />

    <!-- Internal app files -->
    <files-path name="app_files" path="." />

    <!-- Cache directory -->
    <cache-path name="cache" path="." />

    <!-- External cache -->
    <external-cache-path name="external_cache" path="." />
</paths>
```

- [ ] 4. 确认通过（green）：`./build.sh android`，`FileProviderPathsTest` 2 测试 PASSED，`BUILD SUCCESSFUL`。
- [ ] 5. 复核消费面（写进 commit 正文）：

```bash
rg --no-ignore -n "getUriForFile" --type kotlin --type java src-tauri/gen | grep -v "/build/"
# 期望唯一输出：generated/RustWebChromeClient.kt:471
rg -n "external-path" src-tauri/gen/android/app/src/main/res/xml/file_paths.xml
# 期望：无输出（exit 1）
```

- [ ] 6. commit：`android: narrow FileProvider paths to app-scoped roots`。

---

### Task 2: Vite 5→6 + vitest 3 + jsdom 升级

⚠️ **与预期不符（细节修正，不改变结论）**：`@vitejs/plugin-react` 已解析到 4.7.0（非 4.2），peer range `^4.2.0 || ^5.0.0 || ^6.0.0 || ^7.0.0` 已支持 Vite 6/7——**无需升级**，保持 `^4.2.1` range 不动。vite 被钉 5.x 的直接原因是 `vitest@2.1.9` 的 peer `"vite": "^5.0.0"`，故两者必须一起升。

### Files
- `package.json:23-36`（devDependencies）
- `bun.lock`
- 只读核对：`vite.config.ts:41-45`（vitest 配置内联）、`tsconfig.node.json`（include 仅 vite.config.ts）

### Interfaces
- **Consumes**: `vitest/config` 的 `defineConfig` 与 `configDefaults.exclude`（vitest 3 未变更）；Vite 6 ESM 函数式配置（现配置已满足）。
- **Produces**: 升级后的 `package.json`/`bun.lock`；`vite.config.ts`、`tsconfig.node.json` **零改动**。
- **明确排除**：React 18、Tailwind 3 不动。

风险预核（已读码）：无快照测试；`vi.*` 使用全部兼容 vitest 3；无 workspace/projects 配置。

### 步骤
- [ ] 1. 基线：`bun run test`，记录通过数（期望全绿；若有既有红测试，停止并上报）。
- [ ] 2. 升级：

```bash
bun add -d vite@^6 vitest@^3 jsdom@latest
```

- [ ] 3. 核对 diff：`git diff package.json` 期望仅三行变化（plugin-react 行不变）；`grep -oE '"(vite|vitest|jsdom)@[0-9.]+"' bun.lock | sort -u` 期望 `vite@6.`、`vitest@3.`、`jsdom@2x.` 且无 `vite@5.` 残留。
- [ ] 4. `bun run test` 全绿（与基线同数量级；vitest 3 破坏性失败按报错修对应测试文件，预期零修改）。
- [ ] 5. `./build.sh windows android`（验证 Vite 6 生产构建 + 双平台）。
- [ ] 6. commit：`build: upgrade vite 6 / vitest 3 / jsdom (dev toolchain)`。

回滚（步骤 4/5 失败且不可快速修复时）：`git checkout -- package.json bun.lock && bun install`。

---

### Task 3: NN 依赖下载完整性校验（sha256）

### Files
- `scripts/fetch-nn-deps.sh`（五处无校验下载：`:42` DirectML ORT nupkg、`:59` DirectML.dll nupkg、`:66` Linux ORT tgz、`:79` Android ORT-QNN aar、`:85` QNN runtime aar）
- `scripts/nn-versions.env:14-15`（版本变量）
- 新建 `scripts/nn-deps.sha256`

### Interfaces
- **Consumes**: URL 均为 bash 展开后的字面量，清单以**展开后的完整 URL** 为键；格式 `<sha256-hex>␣␣<url>`（两空格），`#` 注释行天然跳过。
- **Produces**: `verify_sha256 <file> <url>` bash 函数（awk 精确匹配 → sha256sum 比对 → 失配/缺条目 exit 1）；五处下载点各加一次校验。
- **唯一允许的占位**：五个 `__FILL_ME__` 哈希（网络获取物），用 `curl -sL <url> | sha256sum` 生成后回填，commit 前必须全部回填。

### 步骤
- [ ] 1. 新建 `scripts/nn-deps.sha256`：

```
# CameraFTP - A Cross-platform FTP companion for camera photo transfer
# Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# sha256 manifest for artifacts downloaded by scripts/fetch-nn-deps.sh.
# Format: <sha256-hex>  <expanded-url>
# URLs must match exactly what fetch-nn-deps.sh curls AFTER nn-versions.env
# expansion. To (re)generate an entry:
#   curl -sL <url> | sha256sum
# When bumping NN_DEMOSAIC_VERSION / QNN_RUNTIME_VERSION / DIRECTML_VERSION,
# regenerate the affected entries in the same commit.
__FILL_ME__  https://www.nuget.org/api/v2/package/Microsoft.ML.OnnxRuntime.DirectML/1.24.1
__FILL_ME__  https://www.nuget.org/api/v2/package/Microsoft.AI.DirectML/1.15.4
__FILL_ME__  https://github.com/microsoft/onnxruntime/releases/download/v1.24.1/onnxruntime-linux-x64-1.24.1.tgz
__FILL_ME__  https://repo1.maven.org/maven2/com/microsoft/onnxruntime/onnxruntime-android-qnn/1.24.1/onnxruntime-android-qnn-1.24.1.aar
__FILL_ME__  https://repo1.maven.org/maven2/com/qualcomm/qti/qnn-runtime/2.42.0/qnn-runtime-2.42.0.aar
```

（执行前用 `sed -n '20,90p' scripts/fetch-nn-deps.sh` 核对五个 URL 的实际展开形态，若与上行有出入以脚本为准。）

- [ ] 2. 改 `scripts/fetch-nn-deps.sh`：在 `source "$SCRIPT_DIR/nn-versions.env"` 之后插入：

```bash
SHA256_MANIFEST="$SCRIPT_DIR/nn-deps.sha256"

# Verify a downloaded artifact against scripts/nn-deps.sha256 (exact-URL match).
# Exits 1 on mismatch or missing entry — never extract an unverified archive.
verify_sha256() {
    local file="$1" url="$2"
    local expected
    expected="$(awk -v u="$url" '$2 == u { print $1; exit }' "$SHA256_MANIFEST")"
    if [[ -z "$expected" ]]; then
        echo "ERROR: no sha256 entry for $url in $SHA256_MANIFEST" >&2
        echo "       Append one: curl -sL '$url' | sha256sum" >&2
        exit 1
    fi
    local actual
    actual="$(sha256sum "$file" | awk '{print $1}')"
    if [[ "$actual" != "$expected" ]]; then
        echo "ERROR: sha256 mismatch for $url" >&2
        echo "  expected: $expected" >&2
        echo "  actual:   $actual" >&2
        exit 1
    fi
    echo "sha256 OK: $(basename "$file")"
}
```

  然后五个下载块各改为"URL 变量化 + curl 后立即校验（在解压之前）"，模式统一为：

```bash
    <NAME>_URL="<展开后的完整URL>"
    curl -fL "$<NAME>_URL" -o "<缓存文件>"
    verify_sha256 "<缓存文件>" "$<NAME>_URL"
```

  五个块的目标形态（块号对应原 `:41-56`/`:57-62`/`:65-69`/`:78-83`/`:84-89`）：

```bash
# 块1
if [[ ! -f "$ORT_WIN_DIR/.directml-stamp" ]]; then
    ORT_DML_URL="https://www.nuget.org/api/v2/package/Microsoft.ML.OnnxRuntime.DirectML/$ORT_DIRECTML_VERSION"
    curl -fL "$ORT_DML_URL" -o "$CACHE_DIR/onnxruntime-directml.nupkg.zip"
    verify_sha256 "$CACHE_DIR/onnxruntime-directml.nupkg.zip" "$ORT_DML_URL"
```

```bash
# 块2
if [[ ! -f "$CACHE_DIR/DirectML.dll" ]]; then
    DIRECTML_URL="https://www.nuget.org/api/v2/package/Microsoft.AI.DirectML/$DIRECTML_VERSION"
    curl -fL "$DIRECTML_URL" -o "$CACHE_DIR/directml.nupkg.zip"
    verify_sha256 "$CACHE_DIR/directml.nupkg.zip" "$DIRECTML_URL"
    unzip -qo -j "$CACHE_DIR/directml.nupkg.zip" "bin/x64-win/DirectML.dll" -d "$CACHE_DIR"
fi
```

```bash
# 块3
if [[ ! -d "$CACHE_DIR/onnxruntime-linux-x64-$ORT_VERSION" ]]; then
    ORT_LINUX_URL="https://github.com/microsoft/onnxruntime/releases/download/v$ORT_VERSION/onnxruntime-linux-x64-$ORT_VERSION.tgz"
    curl -fL "$ORT_LINUX_URL" -o "$CACHE_DIR/onnxruntime-linux-x64-$ORT_VERSION.tgz"
    verify_sha256 "$CACHE_DIR/onnxruntime-linux-x64-$ORT_VERSION.tgz" "$ORT_LINUX_URL"
    tar -xzf "$CACHE_DIR/onnxruntime-linux-x64-$ORT_VERSION.tgz" -C "$CACHE_DIR"
fi
```

```bash
# 块4
if [[ ! -d "$CACHE_DIR/onnxruntime-android-qnn-$ORT_VERSION" ]]; then
    ORT_QNN_URL="https://repo1.maven.org/maven2/com/microsoft/onnxruntime/onnxruntime-android-qnn/$ORT_VERSION/onnxruntime-android-qnn-$ORT_VERSION.aar"
    curl -fL "$ORT_QNN_URL" -o "$CACHE_DIR/ort-android-qnn.aar"
    verify_sha256 "$CACHE_DIR/ort-android-qnn.aar" "$ORT_QNN_URL"
    mkdir -p "$CACHE_DIR/onnxruntime-android-qnn-$ORT_VERSION"
    unzip -qo "$CACHE_DIR/ort-android-qnn.aar" -d "$CACHE_DIR/onnxruntime-android-qnn-$ORT_VERSION"
fi
```

```bash
# 块5
if [[ ! -d "$CACHE_DIR/qnn-runtime-$QNN_RUNTIME_VERSION" ]]; then
    QNN_URL="https://repo1.maven.org/maven2/com/qualcomm/qti/qnn-runtime/$QNN_RUNTIME_VERSION/qnn-runtime-$QNN_RUNTIME_VERSION.aar"
    curl -fL "$QNN_URL" -o "$CACHE_DIR/qnn-runtime.aar"
    verify_sha256 "$CACHE_DIR/qnn-runtime.aar" "$QNN_URL"
    mkdir -p "$CACHE_DIR/qnn-runtime-$QNN_RUNTIME_VERSION"
    unzip -qo "$CACHE_DIR/qnn-runtime.aar" -d "$CACHE_DIR/qnn-runtime-$QNN_RUNTIME_VERSION"
fi
```

（每个块保留原有的其余行不动，仅插入 URL 变量与校验调用；变量名/条件以脚本现状为准。）

- [ ] 3. 确认失败（red，占位哈希即负样本）：`bash -n scripts/fetch-nn-deps.sh && rm -rf /tmp/opencode/nn-cache-red && bash scripts/fetch-nn-deps.sh /tmp/opencode/nn-cache-red; echo "exit=$?"`。期望 `ERROR: no sha256 entry ...` 或 `sha256 mismatch` 且 `exit=1`。
- [ ] 4. 回填哈希（每条一行，约 150MB 一次性下载）：

```bash
curl -sL "https://www.nuget.org/api/v2/package/Microsoft.ML.OnnxRuntime.DirectML/1.24.1" | sha256sum
curl -sL "https://www.nuget.org/api/v2/package/Microsoft.AI.DirectML/1.15.4" | sha256sum
curl -sL "https://github.com/microsoft/onnxruntime/releases/download/v1.24.1/onnxruntime-linux-x64-1.24.1.tgz" | sha256sum
curl -sL "https://repo1.maven.org/maven2/com/microsoft/onnxruntime/onnxruntime-android-qnn/1.24.1/onnxruntime-android-qnn-1.24.1.aar" | sha256sum
curl -sL "https://repo1.maven.org/maven2/com/qualcomm/qti/qnn-runtime/2.42.0/qnn-runtime-2.42.0.aar" | sha256sum
```

- [ ] 5. 确认通过（green）：`rm -rf /tmp/opencode/nn-cache-green && bash scripts/fetch-nn-deps.sh /tmp/opencode/nn-cache-green; echo "exit=$?"`。期望 5 行 `sha256 OK: ...` 且 `exit=0`。
- [ ] 6. 篡改负样本：临时改坏清单第一行哈希首字符，重跑期望 `mismatch ... exit=1`；恢复。
- [ ] 7. 静态检查：`bash -n scripts/fetch-nn-deps.sh`；装了 shellcheck 则 `shellcheck scripts/fetch-nn-deps.sh`。
- [ ] 8. commit（**绝不提交 `__FILL_ME__`**）：`scripts: verify sha256 of downloaded NN deps`。

---

### Task 4: 版本四文件同步脚本（bump + check）

### Files
- 新建 `scripts/bump-version.sh`、`scripts/check-versions.sh`
- 只读核对：`package.json:4`、`src-tauri/Cargo.toml:3`、`src-tauri/tauri.conf.json:5`、`README.md:5` badge、`src-tauri/Cargo.lock`（cameraftp 条目；已核实 bindings/ 与 src/ 均不含版本派生物，gen-types 的作用仅是刷新 Cargo.lock）
- 更新 `AGENTS.md` "Update Version Number" 节

### Interfaces
- **Produces**: `./scripts/bump-version.sh <X.Y.Z>`（校验 semver → 先跑一致性检查 → 四处 sed → gen-types 刷 Cargo.lock → 打印新值）；`./scripts/check-versions.sh`（四文件 + Cargo.lock 五点断言，不一致 exit 1，供 CI 调用）。

### 步骤
- [ ] 1. 新建 `scripts/check-versions.sh`（`chmod +x`）：

```bash
#!/bin/bash
# CameraFTP - A Cross-platform FTP companion for camera photo transfer
# Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Asserts version consistency across package.json, src-tauri/Cargo.toml,
# src-tauri/tauri.conf.json, the README.md badge and src-tauri/Cargo.lock.
# Exits 1 with a diff report on mismatch. Intended for CI and as the
# pre-flight check inside bump-version.sh.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR/.."

pkg="$(sed -n 's/^  "version": "\(.*\)",$/\1/p' package.json | head -1)"
cargo="$(sed -n 's/^version = "\(.*\)"$/\1/p' src-tauri/Cargo.toml | head -1)"
tauri="$(sed -n 's/.*"version": "\(.*\)",$/\1/p' src-tauri/tauri.conf.json | head -1)"
badge="$(sed -n 's/.*version-\([0-9][0-9.]*\)-blue.*/\1/p' README.md | head -1)"
lock="$(sed -n '/^name = "cameraftp"$/{n;s/^version = "\(.*\)"$/\1/p;}' src-tauri/Cargo.lock)"

if [[ -z "$pkg" ]]; then
    echo "ERROR: could not read version from package.json" >&2
    exit 1
fi

failed=0
check() {
    local label="$1" actual="$2"
    if [[ "$actual" != "$pkg" ]]; then
        echo "MISMATCH ${label}: '${actual:-<not found>}' != package.json '${pkg}'" >&2
        failed=1
    fi
}
check "src-tauri/Cargo.toml" "$cargo"
check "src-tauri/tauri.conf.json" "$tauri"
check "README.md badge" "$badge"
check "src-tauri/Cargo.lock (cameraftp)" "$lock"

if [[ "$failed" -ne 0 ]]; then
    echo "Version drift detected. Fix with: ./scripts/bump-version.sh <X.Y.Z>" >&2
    exit 1
fi
echo "Versions consistent: $pkg (package.json / Cargo.toml / tauri.conf.json / README badge / Cargo.lock)"
```

- [ ] 2. 新建 `scripts/bump-version.sh`（`chmod +x`）：

```bash
#!/bin/bash
# CameraFTP - A Cross-platform FTP companion for camera photo transfer
# Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# Usage: ./scripts/bump-version.sh <X.Y.Z>
# Updates the version in ALL FOUR canonical files (see AGENTS.md), refreshes
# src-tauri/Cargo.lock via gen-types, then prints the new values for review.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR/.."

NEW_VERSION="${1:-}"
if [[ ! "$NEW_VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "ERROR: invalid or missing version '${NEW_VERSION}' (expected X.Y.Z, e.g. 1.9.3)" >&2
    exit 1
fi

OLD_VERSION="$(sed -n 's/^  "version": "\(.*\)",$/\1/p' package.json | head -1)"
if [[ -z "$OLD_VERSION" ]]; then
    echo "ERROR: failed to read current version from package.json" >&2
    exit 1
fi
if [[ "$OLD_VERSION" == "$NEW_VERSION" ]]; then
    echo "Version already $NEW_VERSION — nothing to do"
    exit 0
fi

# Fail fast on pre-existing drift between the canonical files
./scripts/check-versions.sh

echo "Bumping version: $OLD_VERSION -> $NEW_VERSION"

sed -i "s/^  \"version\": \"$OLD_VERSION\",/  \"version\": \"$NEW_VERSION\",/" package.json
sed -i "s/^version = \"$OLD_VERSION\"/version = \"$NEW_VERSION\"/" src-tauri/Cargo.toml
sed -i "s/\"version\": \"$OLD_VERSION\"/\"version\": \"$NEW_VERSION\"/" src-tauri/tauri.conf.json
sed -i "s/version-$OLD_VERSION-blue/version-$NEW_VERSION-blue/" README.md

# Refresh src-tauri/Cargo.lock's cameraftp entry (runs cargo.exe via build.sh)
./build.sh gen-types

echo ""
echo "Updated version references:"
echo "  package.json              : $(sed -n 's/^  "version": "\(.*\)",$/\1/p' package.json | head -1)"
echo "  src-tauri/Cargo.toml      : $(sed -n 's/^version = "\(.*\)"$/\1/p' src-tauri/Cargo.toml | head -1)"
echo "  src-tauri/tauri.conf.json : $(sed -n 's/.*"version": "\(.*\)",$/\1/p' src-tauri/tauri.conf.json | head -1)"
echo "  README.md badge           : version-$(sed -n 's/.*version-\([0-9][0-9.]*\)-blue.*/\1/p' README.md | head -1)-blue"
echo "  src-tauri/Cargo.lock      : $(sed -n '/^name = "cameraftp"$/{n;s/^version = "\(.*\)"$/\1/p;}' src-tauri/Cargo.lock)"
```

- [ ] 3. 负样本：`./scripts/bump-version.sh 1.9` → `ERROR: invalid ... exit=1`；临时篡改 `tauri.conf.json:5` 为 9.9.9 → `check-versions.sh` 报 MISMATCH 且 exit=1 → `git checkout --` 恢复。
- [ ] 4. 正向演练：`check-versions.sh` → `Versions consistent: 1.9.2 ...` exit=0；`bump-version.sh 1.9.3` → `git status --short` 恰好 5 个 M 文件；`bump-version.sh 1.9.2` 回滚 → `git status --short` 空输出；再跑 → `Version already 1.9.2 — nothing to do`。
- [ ] 5. 静态检查：`bash -n` 两脚本；装了 shellcheck 则无告警。
- [ ] 6. 更新 `AGENTS.md` "Update Version Number" 节：正文开头加一行 `**快捷方式**：\`./scripts/bump-version.sh <X.Y.Z>\` 自动完成下表全部四处更新并校验一致性（CI 可用 \`./scripts/check-versions.sh\`）。`
- [ ] 7. commit：`scripts: add bump-version.sh and check-versions.sh`。

---

### Task 5: Android 签名卫生

### Files
- `src-tauri/gen/android/app/build.gradle.kts:150-164`（debug buildType；删除 `:159-163` release signingConfig 复用块；release 侧不动）
- `scripts/build-android.sh:246-286`（`check_or_create_keystore`）
- `.gitignore`（追加 `scripts/.keystore-pass`）

### Interfaces
- **Consumes**: AGP 默认行为——debug 未设 signingConfig 时回落 `~/.android/debug.keystore`；`openssl rand -base64 18` 生成 24 字符随机口令（无空白/反斜杠，Properties 与 keytool 均安全）。
- **Produces**: ① debug 构建不再持有 release 签名；② 口令解析优先级 `KEYSTORE_PASSWORD`（env）> `scripts/.keystore-pass`（本地未跟踪）> 现场随机生成并写入该文件；**不再存在默认口令**。
- ⚠️ 本机已存在 `keystore.properties` + `cameraftp.keystore`（均 gitignored），release 路径不受影响。**需向用户提示：debug APK 签名将从 release 证书切换为 AndroidDebug 证书，旧 debug 包需 `adb uninstall com.gjk.cameraftpcompanion` 后重装。**

### 步骤
- [ ] 1. `build.gradle.kts` debug 块替换为（删除 release signingConfig 复用）：

```kotlin
        getByName("debug") {
            manifestPlaceholders["usesCleartextTraffic"] = "true"
            isDebuggable = true
            isJniDebuggable = true
            isMinifyEnabled = false
            packaging {
                jniLibs.keepDebugSymbols.add("*/arm64-v8a/*.so")
            }
            // Debug 构建回落 AGP 默认 debug keystore（~/.android/debug.keystore），
            // 不再复用 release 签名：避免 release 私钥进入日常调试产物分发链路。
        }
```

- [ ] 2. `scripts/build-android.sh` 的 `check_or_create_keystore` 整体替换为（保留原函数名与调用点 `:405`；`warn/success/info/error` 为脚本既有日志函数，以现状为准）：

```bash
# 签名密钥
# 口令解析优先级：KEYSTORE_PASSWORD 环境变量 > 本地未跟踪文件 scripts/.keystore-pass
# （首次现场生成随机口令时写入，避免明文口令进仓库）> 现场生成（openssl rand -base64 18）。
# 不存在任何默认口令。
check_or_create_keystore() {
    local keystore_path="src-tauri/gen/android/keystore.properties"
    local keystore_file="cameraftp.keystore"
    local pass_file="$SCRIPT_DIR/.keystore-pass"

    if [ -f "$keystore_path" ]; then
        return 0
    fi

    warn "签名配置不存在，创建新的签名密钥..."

    local key_alias="${KEYSTORE_ALIAS:-cameraftp}"
    local key_store_pass
    if [ -n "${KEYSTORE_PASSWORD:-}" ]; then
        key_store_pass="$KEYSTORE_PASSWORD"
    elif [ -f "$pass_file" ]; then
        key_store_pass="$(<"$pass_file")"
    else
        if ! command -v openssl >/dev/null 2>&1; then
            error "无法生成随机口令：需要 openssl，或显式设置 KEYSTORE_PASSWORD 环境变量"
            return 1
        fi
        key_store_pass="$(openssl rand -base64 18)"
        printf '%s\n' "$key_store_pass" > "$pass_file"
        chmod 600 "$pass_file"
        success "已生成随机签名口令并保存到 $pass_file（已被 .gitignore 忽略）"
        warn "请妥善备份 $pass_file：口令丢失后该 keystore 无法再用于签名"
    fi
    local key_pass="${KEY_PASSWORD:-$key_store_pass}"
    local key_dname="${KEYSTORE_DNAME:-CN=CameraFTP, OU=Development, O=GJK, L=Unknown, ST=Unknown, C=CN}"

    local keytool_cmd="${SELECTED_TOOLS[keytool]:-keytool}"
    $keytool_cmd -genkey -v \
        -keystore "$keystore_file" \
        -alias "$key_alias" \
        -keyalg RSA \
        -keysize 2048 \
        -validity 10000 \
        -dname "$key_dname" \
        -storepass "$key_store_pass" \
        -keypass "$key_pass"

    mv "$keystore_file" "src-tauri/gen/android/$keystore_file"

    cat > "$keystore_path" << EOF
storeFile=$keystore_file
storePassword=$key_store_pass
keyAlias=$key_alias
keyPassword=$key_pass
EOF

    success "签名密钥已创建: src-tauri/gen/android/$keystore_file"
    info "密钥信息已保存到: $keystore_path"
}
```

（若原函数内变量名/日志函数与上方有出入——如 `SELECTED_TOOLS` 形态——以脚本现状为准做等价替换，语义不变。）

- [ ] 3. `.gitignore` 在 keystore 相关条目区（`:59-67`）追加一行：`scripts/.keystore-pass`
- [ ] 4. `bash -n scripts/build-android.sh`（无输出 exit 0）。
- [ ] 5. debug 构建验证：`./build.sh --debug android`，然后 `keytool -printcert -jarfile out/*d*.apk | head -3` 期望证书 Owner 含 `CN=Android Debug`。
- [ ] 6. release 冒烟（走既有 properties）：`./build.sh android`，`keytool -printcert -jarfile out/*_nn-demosaic.apk | head -3` 期望 Owner 仍为 `CN=CameraFTP, ...`。
- [ ] 7. （可选演练，需先备份，可声明跳过）`mv keystore.properties{,.bak}` + `mv cameraftp.keystore{,.bak}` → `./build.sh --debug android` 期望日志出现"已生成随机签名口令"且 `scripts/.keystore-pass` 存在、`git status` 不显示 → 还原备份、删 pass 文件、重跑 debug 确认绿。
- [ ] 8. `./build.sh windows android` 终验，commit：`android: stop signing debug builds with release key; random keystore passphrases`。
- [ ] 9. commit 正文/PR 提示用户：旧 debug 安装需 `adb uninstall com.gjk.cameraftpcompanion` 后重装。

---

### Task 6: `mutate_and_persist` 落盘出锁（写路径）

⚠️ **与预期不符（两处，已按代码为准修正，发起方已复核确认）**：
1. 原任务书称"内存已更新、落盘失败报错"。实际代码（`config_service.rs:92-93`）是 `save_to_path(...)?` **先于** `*guard = next_config`——**落盘失败时内存未更新**。本计划严格保留这一真实语义。
2. 原任务书称 `platform/android.rs` 是调用点。实际生产调用点：`commands/config.rs:31/79/100` + `color_grading/jni_bridge.rs:282`（JNI 同步上下文，保留同步版）；`file_index/service.rs:536/645/694` 为测试。

### Files
- `src-tauri/src/config_service.rs`（struct `:21-25`、`new_with_path :45-50`、`mutate_and_persist :78-96`；测试模块追加）
- `src-tauri/src/commands/config.rs`（`save_auth_config_with_service :17-41`、`update_preview_config_with_service :75-85`、`save_config :100`、`save_auth_config :118-127`、`update_preview_config :145`、测试 `:242+`）
- `src-tauri/src/color_grading/jni_bridge.rs:282`（保留同步版 + 注释）
- 依赖核对：tokio 已含 `spawn_blocking` 所需 feature；本方案用 std Mutex，无需新增依赖

### Interfaces
- **Consumes**: 现有 `save_to_path`、`lock_result`、`normalized_for_current_platform`、`validate`（均存在，:86/:88 已见）。
- **Produces**:
  - struct 新增 `persist_lock: Arc<std::sync::Mutex<()>>`（`#[derive(Clone)]` 经 Arc 保持成立）；
  - `pub async fn mutate_and_persist_async<F, R>(&self, mutate: F) -> Result<R, AppError> where F: FnOnce(&mut AppConfig) -> R`——spawn_blocking 落盘，语义与同步版一致（内存仅在落盘成功后换入）；
  - 同步版签名不变，供 JNI/测试使用；两版经 `persist_lock` 互斥（序列化整个 mutate→persist 序列，等价旧的单写锁全程序列化，防两阶段交错丢更新）。

设计要点：内存 RwLock 只在快速阶段（clone+mutate+validate / 换入）短暂持有，fsync 不再阻塞 `get()`；async 版持 std Mutex 跨 `spawn_blocking().await`（被等待任务在 blocking 池执行不需要该锁，取消时 RAII 释放），加 `#[allow(clippy::await_holding_lock)]` 并注释论证。

### 步骤
- [ ] 1. 写失败测试（`config_service.rs` `mod tests` 追加三个；第一个是**语义钉死测试**——当前代码下应通过；第二、三个因 `mutate_and_persist_async` 不存在而编译失败即 red）：

```rust
    #[test]
    fn mutate_and_persist_failure_leaves_memory_unchanged() {
        // 语义钉死：save_to_path 在 blocked-parent（文件而非目录）上
        // create_dir_all 失败 → 落盘失败 ⇒ 内存保持旧值。同步/异步两版
        // 都必须维护这一契约（mutate_and_persist 的 `?` 先于 `*guard` 换入）。
        let temp_dir = tempdir().expect("failed to create temp dir");
        let blocked_parent = temp_dir.path().join("blocked-parent");
        std::fs::write(&blocked_parent, "not a directory").expect("failed to create blocker file");

        let service = ConfigService::new_with_path(blocked_parent.join("config.json"));

        let result = service.mutate_and_persist(|config| config.port = 7073);

        assert!(result.is_err());
        assert_eq!(
            service.get().expect("failed to get config").port,
            AppConfig::default().port
        );
    }

    #[tokio::test]
    async fn mutate_and_persist_async_keeps_memory_unchanged_on_persist_failure() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let blocked_parent = temp_dir.path().join("blocked-parent");
        std::fs::write(&blocked_parent, "not a directory").expect("failed to create blocker file");

        let service = ConfigService::new_with_path(blocked_parent.join("config.json"));

        let result = service
            .mutate_and_persist_async(|config| config.port = 7074)
            .await;

        assert!(result.is_err());
        assert_eq!(
            service.get().expect("failed to get config").port,
            AppConfig::default().port
        );
    }

    #[tokio::test]
    async fn concurrent_gets_do_not_deadlock_with_mutate_and_persist_async() {
        // 正确性冒烟（非时延断言）：100 个并发 get 与一次落盘变更共存，
        // 全部完成且最终 get 看到新值（耗时阈值断言在 CI 上过于 flaky）。
        let temp_dir = tempdir().expect("failed to create temp dir");
        let config_path = temp_dir.path().join("config.json");
        let service = Arc::new(ConfigService::new_with_path(config_path));
        service.load().expect("failed to load config");

        let mut readers = Vec::new();
        for _ in 0..100 {
            let reader = Arc::clone(&service);
            readers.push(tokio::spawn(async move {
                reader.get().expect("concurrent get must not fail")
            }));
        }

        service
            .mutate_and_persist_async(|config| config.port = 7075)
            .await
            .expect("failed to mutate and persist config");

        for handle in readers {
            handle.await.expect("reader task must not deadlock");
        }

        assert_eq!(service.get().expect("failed to get config").port, 7075);
    }
```

- [ ] 2. 确认失败（red）：`cargo.exe test --lib -- config_service` → 编译错误 `no method named mutate_and_persist_async`（语义钉死测试通过作为基线）。
- [ ] 3. 最小实现：
  - struct 加 `persist_lock: Arc<std::sync::Mutex<()>>`（含文档注释说明序列化不变量）；
  - `new_with_path` 构造处初始化；
  - 同步版 `mutate_and_persist` 整体替换为：

```rust
    pub fn mutate_and_persist<F, R>(&self, mutate: F) -> Result<R, AppError>
    where
        F: FnOnce(&mut AppConfig) -> R,
    {
        // Serialize the whole mutate→persist sequence; see persist_lock docs.
        // 失败语义：validate 失败或落盘失败时内存保持旧值（`?` 先于换入）。
        let _persist_guard = lock_result(self.persist_lock.lock())?;

        let (next_config, result) = {
            let mut guard = lock_result(self.config.write())?;
            let mut next_config = guard.clone();
            let result = mutate(&mut next_config);
            next_config = next_config.normalized_for_current_platform();

            if let Err(e) = next_config.validate() {
                return Err(AppError::Other(format!("Invalid configuration: {}", e)));
            }

            (next_config, result)
        }; // write guard released here — disk I/O below no longer blocks get()

        Self::save_to_path(&self.config_path, &next_config)?;

        let mut guard = lock_result(self.config.write())?;
        *guard = next_config;

        Ok(result)
    }
```

  - 紧随其后新增异步版：

```rust
    /// Async variant of [`ConfigService::mutate_and_persist`]: identical
    /// semantics (the in-memory config is only swapped in after the persist
    /// succeeded), but the fsync-heavy `save_to_path` runs on tokio's
    /// blocking pool so Tauri async commands do not stall worker threads.
    // persist_guard is a std Mutex held across the spawn_blocking await on
    // purpose: the awaited task runs on the blocking pool and never needs
    // this lock back; cancellation drops the guard via RAII. See the field
    // docs for the serialization invariant.
    #[allow(clippy::await_holding_lock)]
    pub async fn mutate_and_persist_async<F, R>(&self, mutate: F) -> Result<R, AppError>
    where
        F: FnOnce(&mut AppConfig) -> R,
    {
        let _persist_guard = lock_result(self.persist_lock.lock())?;

        let (next_config, result) = {
            let mut guard = lock_result(self.config.write())?;
            let mut next_config = guard.clone();
            let result = mutate(&mut next_config);
            next_config = next_config.normalized_for_current_platform();

            if let Err(e) = next_config.validate() {
                return Err(AppError::Other(format!("Invalid configuration: {}", e)));
            }

            (next_config, result)
        };

        let path = self.config_path.clone();
        tokio::task::spawn_blocking(move || Self::save_to_path(&path, &next_config))
            .await
            .map_err(|e| AppError::Other(format!("Config persist task failed: {}", e)))??;

        let mut guard = lock_result(self.config.write())?;
        *guard = next_config;

        Ok(result)
    }
```

- [ ] 4. 确认通过（green）：`cargo.exe test --lib -- config_service` → 全绿（含既有测试与新增 3 个）。
- [ ] 5. 迁移 async 可达调用点（`commands/config.rs`）：
  - `save_config`（`:100`）：`mutate_and_persist(...)?` → `mutate_and_persist_async(...).await?`（闭包体不动，见跨批次衔接注记 2）；
  - `update_preview_config_with_service`（`:75-85`）改 `async fn` + `.await`；调用点 `update_preview_config`（`:145`）加 `.await`；
  - `save_auth_config_with_service`（`:17-41`）改 `async fn`，Argon2 哈希保持在 `spawn_blocking` 内、落盘走 `mutate_and_persist_async`：

```rust
pub async fn save_auth_config_with_service(
    config_service: &ConfigService,
    anonymous: bool,
    username: String,
    password: String,
) -> Result<(), AppError> {
    let password_hash = tokio::task::spawn_blocking(move || {
        crate::crypto::hash_password(&password)
    })
    .await
    .map_err(|e| AppError::Other(format!("hash task failed: {}", e)))??;

    config_service
        .mutate_and_persist_async(move |config| {
            config.advanced_connection.auth.anonymous = anonymous;
            config.advanced_connection.auth.username = username;
            config.advanced_connection.auth.password_hash = password_hash;
        })
        .await?;
    Ok(())
}
```

    （字段赋值路径以该函数现有实现为准；若批次1任务3已把命令层做成 spawn_blocking 包裹，删除包裹层改为直接 `.await` 调用本 helper——见跨批次衔接注记 1。）
  - 同文件测试适配：涉及 helper 的 `#[test]` → `#[tokio::test] async fn` 并 `.await`；
  - `color_grading/jni_bridge.rs:282` 保留同步 `mutate_and_persist`，上方加注释：`// JNI 同步上下文（无 tokio runtime），用同步版；persist_lock 保证与 async 命令互斥`。
- [ ] 6. `cargo.exe test --lib`（全量 Rust）→ 全绿。
- [ ] 7. `./build.sh windows android` 终验。
- [ ] 8. commit：`config: move config persistence off the in-memory lock (async variant)`。

---

### Task 7: `ConfigService::get()` 返回 `Arc` 快照（读路径）

在任务 6 之后实施（同一文件，基于任务 6 完成后的代码）。

### Files
- `src-tauri/src/config_service.rs`（struct、`load :52-58`、`get :60-63`、`get_or_default :68-76`、任务6改后的两个 mutate）
- 热路径调用点（每上传一张照片各触发一次整份 clone）：`ai_edit/service.rs:99-102`、`color_grading/service.rs:158-159`、`auto_open/service.rs:229-235`（Windows）
- 需适配的"字段移出"调用点：`commands/config.rs:105`、`commands/config.rs:367-371`（测试）、`file_index/service.rs:31-44`、`auto_open/service.rs:229-235`
- 无需改动（纯借用，Arc 自动解引用）：`ai_edit/service.rs:383-386`、`color_grading/jni_bridge.rs:242-253`、`color_grading/service.rs:160-163`、`ftp/server_factory.rs:117-178`；`get_or_default` 保持 `-> AppConfig` 签名，其调用点零改动

### Interfaces
- **Produces**: `pub fn get(&self) -> Result<Arc<AppConfig>, AppError>`（clone 仅原子计数 +1）；快照不可变——后续 mutate 通过替换内部 `Arc` 生效，绝不原地改写旧快照。ts-rs 导出不受影响（gen-types 产物应零 diff）。

### 步骤
- [ ] 1. 快照语义钉死测试（重构前后都应通过；red 阶段由第 3 步签名编译错误承担）：

```rust
    #[test]
    fn get_snapshot_is_stable_across_mutation() {
        // get() 返回不可变快照：后续 mutate 通过替换内部 Arc 生效，
        // 绝不原地修改旧快照。若实现退化为原地 patch，本测试失败。
        let temp_dir = tempdir().expect("failed to create temp dir");
        let config_path = temp_dir.path().join("config.json");
        let service = ConfigService::new_with_path(config_path);
        service.load().expect("failed to load config");

        let default_port = service.get().expect("failed to get config").port;
        let snapshot = service.get().expect("failed to get snapshot");

        service
            .mutate_and_persist(|config| config.port = 7076)
            .expect("failed to mutate and persist config");

        assert_eq!(snapshot.port, default_port, "old snapshot must be immutable");
        assert_eq!(
            service.get().expect("failed to get config").port,
            7076,
            "new snapshot must reflect the mutation"
        );
    }
```

- [ ] 2. `cargo.exe test --lib -- config_service` 确认通过（基线）。
- [ ] 3. 最小实现（编译器驱动重构）：
  - struct：`config: Arc<RwLock<Arc<AppConfig>>>`；`new_with_path` 初始化 `Arc::new(RwLock::new(Arc::new(AppConfig::default())))`；
  - `load`：`*guard = loaded_config;` → `*guard = Arc::new(loaded_config);`
  - `get` 替换为：

```rust
    /// Cheap snapshot: clones only an `Arc`, not the whole `AppConfig`.
    /// The snapshot is immutable — later mutations replace the inner Arc
    /// and never mutate an outstanding snapshot in place.
    pub fn get(&self) -> Result<Arc<AppConfig>, AppError> {
        let guard = lock_result(self.config.read())?;
        Ok(Arc::clone(&guard))
    }
```

  - `get_or_default` 保持 `-> AppConfig`：`Ok(config) => (*config).clone(),`（冷路径一次性 clone，换三个调用点零改动）；
  - 两个 mutate 版本中 `guard.clone()` → `(*guard).clone()`；`*guard = next_config;` → `*guard = Arc::new(next_config);`
- [ ] 4. 适配"字段移出"调用点：
  - `commands/config.rs:105`：`config_service.get()?.save_path` → `config_service.get()?.save_path.clone()`（若批次2已改写 save_config，位置在其新版内，同模式）
  - `commands/config.rs:367-371`（测试）：`.preview_config` 移出改 `.preview_config.clone()`
  - `file_index/service.rs:31-44` `new()`：

```rust
    pub fn new(config_service: Arc<ConfigService>) -> Self {
        let config = config_service.get().unwrap_or_else(|e| {
            warn!(error = %e, "Failed to read config from ConfigService, using defaults");
            Arc::new(AppConfig::default())
        });
        Self {
            index: RwLock::new(FileIndex::new()),
            save_path: RwLock::new(config.save_path.clone()),
            #[cfg(target_os = "windows")]
            watcher: Mutex::new(Some(FileWatcher::new(config.save_path.clone()))),
            app_handle: Arc::new(RwLock::new(None)),
        }
    }
```

  - `auto_open/service.rs:229-235`（Windows cfg）：

```rust
    fn current_config(&self) -> PreviewWindowConfig {
        self.config_service
            .get()
            .map(|c| c.preview_config.clone().unwrap_or_default())
            .unwrap_or_default()
    }
```

  - 按编译器指引扫尾（预期"无需改动"清单如上；遗漏点按同模式最小适配，禁止借机重构）。
- [ ] 5. 确认通过：`cargo.exe test --lib`（全量）→ 全绿。
- [ ] 6. `./build.sh windows android` 终验；`git status --short src-tauri/bindings/` 期望**无输出**（类型未变，bindings 零 diff）。
- [ ] 7. commit：`config: return Arc snapshot from ConfigService::get to kill per-file deep clones`。

---

## 批次收尾

- [ ] 全部 7 个任务完成后：`./build.sh windows android` 一次性终验 + `bun run test` 全绿 + `git log --oneline` 确认 7 个独立 commit。
- [ ] 汇报两处"以代码为准"的修正（任务2 plugin-react 无需升级；任务6 失败语义与调用点清单）及任务5的用户提示（debug 包需卸载重装）。

## 风险与回退

- 每任务独立 commit，可单独 revert。
- 任务 2 附带回滚命令（`git checkout -- package.json bun.lock && bun install`）。
- 任务 5 改变 debug 签名（预期内，见提示）；release 路径不受影响。
- 任务 6/7 为同文件连续重构，若 7 出现问题可单独 revert 7 保留 6（接口向后兼容：`Arc` 解引用透明）。
