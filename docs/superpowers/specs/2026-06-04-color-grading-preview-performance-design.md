# Color Grading Real-time Preview Performance Optimization

> CameraFTP Android — 调色实时预览性能优化设计文档
>
> 2026-06-04

---

## 1. Problem Statement

Android 调色实时预览延迟过高（每帧 500ms–2s），远达不到"实时"状态。
当前数据流为：

```
滑块变化 → JS(requestApply) → @JavascriptInterface → Kotlin Thread → JNI
  → Rust block_on → C++ 全分辨率 RAW→JPEG 编码(500ms–2s)
  → 写磁盘 → Kotlin FileInputStream → WebView img.src
```

核心瓶颈依次为：**全分辨率 JPEG 重编码 > 磁盘 I/O > 无节流导致请求堆积 > JPEG 质量偏高**。

---

## 2. Optimization Items

### 2.1 低分辨率预览（核心，预期收益最大）

**问题**：每次 apply 都以原始分辨率（通常 4000–6000px）做 JPEG 编码。编码时间与像素数成正比。

**方案**：C++ 在 JPEG 编码前等比缩放到 fit 在 `maxWidth × maxHeight` 范围内。
Kotlin 侧获取设备屏幕分辨率，传入 `maxWidth=screenWidth, maxHeight=screenHeight`。
横拍照片由高度限制，竖拍由宽度限制，比例始终正确。

**各层改动**：

| 层 | 文件 | 改动 |
|----|------|------|
| C++ | `raApplyPreviewGrading` | 新增 `int maxWidth, int maxHeight` 参数（为 0 时保持原分辨率）。内部：`scale = min(maxWidth / w, maxHeight / h)`，等比缩放后做 JPEG 编码 |
| Rust FFI | `color_grading/ffi.rs` | `RaApplyPreviewGradingFn` 签名增加 `c_int, c_int`；`apply_preview_grading()` 方法增加 `max_width: u32, max_height: u32` |
| Rust preview | `color_grading/preview.rs` | `apply()` 方法增加 `max_width: u32, max_height: u32`，透传至 FFI |
| Rust JNI | `color_grading/jni_bridge.rs` | `nativeApplyPreview` 增加 `jint maxWidth, jint maxHeight` |
| Kotlin JNI Bridge | `bridges/ColorGradingJniBridge.kt` | `nativeApplyPreview` 声明增加 `maxWidth: Int, maxHeight: Int`；`applyPreview()` 增加对应参数 |
| Kotlin Activity | `ColorGradingActivity.kt` | 在 `NativeColorGradingPreviewBridge.applyPreview()` 中通过 `activity.resources.displayMetrics` 获取屏幕宽高，传给 JNI |

**预期的延迟降低**：分辨率从 6000px 降到 800px，像素数降至 1/56，JPEG 编码从 ~1s 降到 ~20ms。

---

### 2.2 JPEG 预览质量降低

**问题**：当前 `PREVIEW_JPEG_QUALITY = 80`（`preview.rs:14`），预览只需看清色调，不需要高保真。

**方案**：`PREVIEW_JPEG_QUALITY` 从 80 降到 50。

**改动**：仅 `color_grading/preview.rs` 第 14 行，常量改值。

**预期的延迟降低**：JPEG 编码速度提升约 30%。

---

### 2.3 内存 buffer 传输，消除磁盘 I/O

**问题**：当前 apply 流程为：C++ 写 JPEG 文件 → Rust 返回文件路径 → Kotlin `FileInputStream` 读文件 → WebView 渲染。每次 apply 有一次磁盘写+读。

**方案**：C++ 将 JPEG 编码到内存 buffer 返回，Rust → JNI → Kotlin → WebView 全程内存传输。

**C++ 侧**：修改 `raApplyPreviewGrading`。输出方式从文件路径改为内存 buffer——分配 buffer 写入 JPEG 字节，通过 `outBuffer`（`unsigned char**`）和 `outLen`（`int*`）返回。新增 `raFreePreviewBuffer(unsigned char* buffer)` 用于释放。

**各层改动**：

| 层 | 文件 | 改动 |
|----|------|------|
| C++ | `raApplyPreviewGrading` | 签名变更：最后一个参数从 `const char* outputPath` 改为 `unsigned char** outBuffer, int* outLen`。内部用内存编码代替文件写入 |
| C++ | 新增 | `void raFreePreviewBuffer(unsigned char* buffer)` |
| Rust FFI | `color_grading/ffi.rs` | `RaApplyPreviewGradingFn` 签名改为 `*mut *mut u8, *mut c_int` 替代 `*const c_char`。`apply_preview_grading()` 返回 `Result<Vec<u8>>`，内部分配 buffer、调用 FFI、拷贝到 Vec |
| Rust preview | `color_grading/preview.rs` | `apply()` 返回 `Result<Vec<u8>>` 而非 `Result<String>`。移除 `preview_output_path` 字段和 `ActiveSession` 中的磁盘路径。移除预览目录创建和 JPEG 文件清理逻辑 |
| Rust JNI | `color_grading/jni_bridge.rs` | `nativeApplyPreview` 返回 JSON 中字段从 `url` 变为 `buffer`（Base64 编码的 JPEG 字节） |
| Kotlin JNI Bridge | `bridges/ColorGradingJniBridge.kt` | `parseResultWithUrl` 改为 `parseResultWithBuffer`，解析 `buffer` 字段并 Base64 解码为 `ByteArray`。`applyPreview()` 返回 `Result<ByteArray>` |
| Kotlin Activity | `ColorGradingActivity.kt` | `previewFilePath: @Volatile String?` 改为 `previewJpegBytes: @Volatile ByteArray?`（只替换引用，不修改数组内容，`@Volatile` 足够）。`shouldInterceptRequest` 改为用 `ByteArrayInputStream(previewJpegBytes)` 替代 `FileInputStream`。`extractFilePathFromUrl` 删除 |

---

### 2.4 滑块节流（Throttle）

**问题**：当前 JS 侧只有布尔门控 `applyPending`，无时间节流。快速拖动滑块时，每个 step 都会排队，最后一个到达时可能已经生成了多帧无用的中间帧。

**方案**：JS 侧增加 50ms throttle。拖动过程中每 50ms 最多发起一次 apply；拖动停止时立即发起最后一次。

**改动**：仅 `color_grading_preview.html` 的 `requestApply()` 逻辑。

```
当前逻辑：
  applyPending=true  → 发起 → 收到回调后 applyPending=false

新逻辑：
  throttle timer (50ms) → timer 到期后发起 apply
  滑块变化时: 重置 timer
  松手时: 立即取消 timer 并直接发起 apply
```

**具体实现**：用 `setTimeout`/`clearTimeout` 配合 `<input type="range">` 的 `input` 事件（持续拖动）+ `change` 事件（松手时自动触发）实现 throttle + trailing edge fire。

---

## 3. New Data Flow

优化后的数据流（变更部分标 **→**）：

```
滑块变化 → JS(throttle 50ms → debounced requestApply)
  → @JavascriptInterface → Kotlin Thread
  → JNI(nativeApplyPreview) → Rust block_on
  → C++: 内存缩放到 screenWidth×screenHeight → JPEG quality=50 → 编码到内存 buffer
  → JNI 返回 Base64 JPEG bytes → Kotlin 解码为 ByteArray → 存入 @Volatile 字段
  → WebViewClient.shouldInterceptRequest: ByteArrayInputStream(previewJpegBytes)
  → WebView img.src 刷新（约每 50ms 一次）
  消除: 磁盘 I/O（写+读）、全分辨率编码、无节流堆积
```

---

## 4. Files Changed Summary

| 文件 | 层 | 改动量 |
|------|-----|--------|
| C++ `raw_alchemy_core` (外部仓库) | C++ | 中 — 修改 `raApplyPreviewGrading` 签名和实现，新增 `raFreePreviewBuffer` |
| `src-tauri/src/color_grading/ffi.rs` | Rust FFI | 中 — 函数指针签名、`apply_preview_grading()` 方法 |
| `src-tauri/src/color_grading/preview.rs` | Rust preview | 中 — `apply()` 签名、`ActiveSession` 字段、文件 I/O 逻辑移除 |
| `src-tauri/src/color_grading/jni_bridge.rs` | Rust JNI | 小 — JSON 返回字段 `url`→`buffer`，新增 `maxWidth/maxHeight` |
| `src-tauri/gen/android/.../bridges/ColorGradingJniBridge.kt` | Kotlin JNI | 小 — `external fun` 签名，`parseResult` 逻辑 |
| `src-tauri/gen/android/.../ColorGradingActivity.kt` | Kotlin Activity | 中 — `previewFilePath`→`previewJpegBytes`，`shouldInterceptRequest`，`displayMetrics` |
| `src-tauri/gen/android/.../color_grading_preview.html` | JS | 小 — throttle 逻辑 |

---

## 5. Testing Strategy

| 场景 | 验证点 |
|------|--------|
| 滑块拖动 | 预览画面在 50ms throttle 内更新，无明显卡顿 |
| 滑块松手 | 最后一帧在 100ms 内到达（trailing edge） |
| 横拍照片 | 预览宽度占满屏幕，高度等比缩放 |
| 竖拍照片 | 预览高度占满屏幕，宽度等比缩放 |
| LUT 快速切换 | 每次切换触发一次 apply，preview 正确更新 |
| 保存按钮 | 保存操作不受影响，endPreview + 触发生成高质量 JPEG 批次 |
| 内存泄漏 | 多次 apply 后无内存增长（buffer 正确释放） |
| 不同分辨率设备 | 1080p / 720p 屏幕各自使用适配的 maxWidth/maxHeight |
| 取消预览 | onDestroy 中正确释放 C++ 内存和 Kotlin 引用 |

---

## 6. Risks & Mitigations

| 风险 | 缓解措施 |
|------|----------|
| C++ 签名变更导致 Android 已有 .so 不兼容 | C++ 库由项目控制，同步重新编译 .so 并更新 app 中的 `jniLibs` 即可 |
| Base64 encoding 增加内存开销（100KB JPEG → ~133KB Base64） | 可接受。相比磁盘 I/O 节省的时间（几 ms vs 几十 ms），Base64 编码/解码（微秒级）净收益为正 |
| `@Volatile var ByteArray?` 多线程读写安全性 | 线程安全。写线程（background）→ 读线程（WebView intercept on UI thread），`@Volatile` 保证引用可见性。每次写入分配新数组，不修改已有内容 |
| 屏幕分辨率可能在预览过程中变化（旋转） | 在 `beginPreview()` 时获取一次，预览期间不变。如需支持旋转，可在 `onConfigurationChanged` 中更新 |
