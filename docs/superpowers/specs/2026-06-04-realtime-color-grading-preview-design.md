# Real-time Color Grading Preview — Design Spec

**Date:** 2026-06-04
**Status:** Draft
**Scope:** Android ImageViewerActivity 调色按钮触发实时预览模式

## 1. Overview

将 Android 端图片查看器（ImageViewerActivity）的"调色"功能从对话框模式升级为实时预览模式。用户点击调色后，打开一个全屏的调色专用 Activity，内嵌 WebView 显示实时预览图和调参控件。每次参数变更后，立即刷新预览图。

**不在本次范围内：**
- Windows PreviewWindow 的调色功能（保持对话框模式）
- Android 图库界面的批量调色（保持对话框模式）
- 设置界面的自动调色配置（保持现有逻辑）

## 2. Architecture

### 2.1 新增 ColorGradingActivity

新建 `ColorGradingActivity`（继承 `AppCompatActivity`），从 `ImageViewerActivity` 通过 `startActivity` 启动，携带 `Intent` extra：

```kotlin
intent.putExtra("filePath", resolvedFilePath)
intent.putExtra("displayName", currentDisplayName)
```

Activity 内部包含：
- **全屏 WebView**：加载 `color_grading_preview.html` 资产文件
- **shouldInterceptRequest**：拦截自定义 URL scheme `preview://latest`，从 Rust 写入的临时 JPEG 文件读取并返回 `FileInputStream`

### 2.2 IPC 通信路径

保持现有转发模式，不注册新的 Tauri IPC 通道。路径：

```
ColorGradingActivity WebView
  → NativeBridge (JavascriptInterface)
  → Kotlin evaluateJavascript
  → MainActivity Tauri WebView
  → window.__tauriXxx() JS functions
  → invoke() → Tauri IPC → Rust
```

需要在 `App.tsx` 中新增以下全局 JS 函数供 Kotlin 调用：

| 全局函数 | 作用 |
|---------|------|
| `__tauriBeginColorGradingPreview(filePath)` | 调用 `invoke('begin_color_grading_preview', { imagePath })` |
| `__tauriApplyColorGradingPreview(lutId, meteringMode, evOffset)` | 调用 `invoke('apply_color_grading_preview', { ... enableLensCorrection: true })` → 返回文件路径 |
| `__tauriEndColorGradingPreview()` | 调用 `invoke('end_color_grading_preview')` |

Kotlin 端通过 `evaluateJavascript` 调用这些函数，通过 `Promise` 回调获取返回值。

### 2.3 图片显示方案

采用 `WebViewClient.shouldInterceptRequest` + 自定义 URL scheme：

- Rust 端 `apply_color_grading_preview` 将 JPEG 写入临时文件（现有行为不变）
- Kotlin 端保存返回的文件路径
- WebView 中 `<img src="preview://latest?ts=xxx">` 触发请求
- `shouldInterceptRequest` 拦截 `preview://` scheme，用 `FileInputStream` 流式返回 JPEG
- 每次调参后，JS 更新 `img.src` 添加新时间戳以触发刷新

**方案选择理由：**
- 零额外编码开销（无 base64）
- Android 15+ 完全支持（`shouldInterceptRequest` 未被弃用）
- 大文件（30MB+）通过 `FileInputStream` 直接流式传输，不会 OOM
- 自定义 scheme 不影响安全上下文需求（调色页面不使用 crypto/Service Worker）

### 2.4 保存与导出

保存时调用完整 `process_file` 流程（非 preview）。执行顺序：

1. Kotlin 调用 `__tauriEndColorGradingPreview()` 关闭 Rust preview session + 删除临时文件
2. Kotlin 调用 `__tauriTriggerColorGrading(filePath, lutId, meteringMode, evOffset, false)` 加入后台处理队列
3. Kotlin 调用 `__tauriSaveColorGradingLastUsed(lutId, meteringMode, evOffset)` 保存配置
4. 立即 `finish()` 关闭 `ColorGradingActivity`，返回 `ImageViewerActivity`
5. 后台 `process_file` 完成后，通过现有事件链将结果插入查看器列表

### 2.5 临时文件管理

- Rust preview session 期间，所有 apply 操作覆盖写入同一个临时文件 `preview_{session_ptr}.jpg`
- `end_color_grading_preview` 时删除临时文件 + 释放 C++ session
- 用户中途返回（Activity 销毁）时，Kotlin 在 `onDestroy` 中调用 `endPreview` 确保清理

## 3. UI Design

### 3.1 页面结构

`color_grading_preview.html` 是一个全屏暗色调页面，分为三个区域：

```
┌─────────────────────────────────────┐
│  ← 返回          调色               │  ← 顶部导航栏
├─────────────────────────────────────┤
│                                     │
│                                     │
│         [RAW 预览图]                 │  ← 预览区域（自适应缩放）
│                                     │
│                                     │
├─────────────────────────────────────┤
│  [LUT: Classic Chrome ▼]           │  ← 控制面板
│  [测光: 高光保护 ▼]                  │
│  曝光偏移           +0.5 EV         │
│  ════════════●═════════════         │
│  [重置]              [保存]          │
└─────────────────────────────────────┘
```

### 3.2 状态机

```
LOADING → READY → ADJUSTING (循环) → SAVING → (关闭)
   ↓         ↓
 (返回)    (返回)
   ↓         ↓
 CANCEL    CANCEL
```

| 状态 | 预览区 | 控制面板 | 顶部返回按钮 |
|------|--------|---------|------------|
| **LOADING** | 旋转动画 + "正在解码 RAW 图片..." | 半透明不可操作 | 可点击（中断解码） |
| **READY** | 显示默认 LUT+曝光 的预览图 | 可操作 | 可点击（结束 session） |
| **ADJUSTING** | 显示当前参数的预览图 | 可操作 | 可点击（结束 session） |
| **SAVING** | 显示最终预览图 | 不可操作（防重复点击） | 不可操作 |

### 3.3 LOADING 状态详情

- WebView 加载完成后，立即通过 `NativeBridge.beginPreview(filePath)` 触发 RAW 解码
- 解码期间显示加载动画（CSS spinner）和提示文字 "正在解码 RAW 图片..."
- 返回按钮可点击：调用 `NativeBridge.cancelPreview()` → Kotlin 调 `__tauriEndColorGradingPreview()` → 关闭 Activity

### 3.4 READY / ADJUSTING 状态

- 解码成功后，立即调用 `NativeBridge.applyPreview(defaultLut, defaultMetering, defaultEvOffset)` 获取默认预览
- 默认值来源：通过 `NativeBridge.getConfig()` 从 Kotlin 获取，Kotlin 从主 WebView 的 `__tauriGetColorGradingLastUsed()` 和 `__tauriGetColorGradingPresets()` 获取

**控件：**
- **LUT 选择**：下拉菜单，选项来自 presets 列表
- **测光模式**：下拉菜单，选项：高光保护、矩阵测光、中央重点测光、平均测光、混合测光
- **曝光偏移**：滑块，范围 -5.0 到 +5.0，步进 0.1
- **重置按钮**：恢复到默认配置（LUT Provia、高光保护、EvOffset=0）
- **保存按钮**：触发保存流程

**实时刷新机制（异步回调模式）：**

由于 `@JavascriptInterface` 方法只能返回 `void`，IPC 异步结果通过 Kotlin→JS 回调传递：

1. 用户修改参数 → JS 调用 `NativeBridge.applyPreview(lutId, meteringMode, evOffset)`
2. `NativeBridge.applyPreview()` 通过 `evaluateJavascript` 调用主 WebView 上的 `__tauriApplyColorGradingPreview()`
3. 主 WebView 返回文件路径字符串（通过 `evaluateJavascript` 的 `ValueCallback<String>` 回调）
4. Kotlin 保存 `previewFilePath`，然后通过 `webView.evaluateJavascript("window.refreshPreview()")` 通知调色 WebView 刷新
5. 调色 WebView 的 `refreshPreview()` 更新 `img.src = "preview://latest?" + Date.now()` 触发 `shouldInterceptRequest`

**防抖：** 如果用户快速连续调整参数，Kotlin 端应取消前一个未完成的 `applyPreview` 请求，仅处理最新的。实现方式：维护一个 `applyRequestId` 计数器，回调中检查是否仍为当前请求。

### 3.5 SAVING 状态

- 保存按钮点击后，禁用所有交互
- 按顺序执行保存流程：
  1. Kotlin 调用 `__tauriEndColorGradingPreview()` 关闭 Rust preview session
  2. Kotlin 调用 `__tauriTriggerColorGrading(filePath, lutId, meteringMode, evOffset, false)` 加入后台处理队列
  3. Kotlin 调用 `__tauriSaveColorGradingLastUsed(lutId, meteringMode, evOffset)` 保存配置
  4. 立即 `finish()` 关闭 Activity（不等待后台处理完成）
- 后台处理完成后，通过现有事件链将结果插入 ImageViewerActivity 列表

## 4. Kotlin Layer

### 4.1 ColorGradingActivity

```
位置: src-tauri/gen/android/.../ColorGradingActivity.kt
```

**职责：**
- 创建全屏 WebView，加载 `color_grading_preview.html`
- 注册 `NativeColorGradingPreviewBridge` (JavascriptInterface)
- 实现 `shouldInterceptRequest` 处理 `preview://` scheme
- 管理生命周期：`onDestroy` 时确保清理 preview session

**WebView 配置：**
```kotlin
settings.javaScriptEnabled = true
settings.domStorageEnabled = false
settings.allowFileAccess = false  // 不需要直接文件访问
```

### 4.2 NativeColorGradingPreviewBridge

```
位置: ColorGradingActivity.kt 内部类
```

**接口方法（@JavascriptInterface）：**

| 方法 | 参数 | 返回 | 作用 |
|------|------|------|------|
| `beginPreview(filePath)` | String | void | 通知 Kotlin 开始 preview session（异步，通过 `onPreviewReady`/`onPreviewError` 回调结果） |
| `applyPreview(lutId, meteringMode, evOffset)` | String, String, Float | void | 通知 Kotlin 应用调色参数（异步，通过 `refreshPreview`/`onPreviewError` 回调结果） |
| `save(lutId, meteringMode, evOffset)` | String, String, Float | void | 通知 Kotlin 保存并关闭（顺序：end → trigger → saveConfig → finish） |
| `cancelPreview()` | 无 | void | 通知 Kotlin 中断/结束 preview |
| `getConfig()` | 无 | String (JSON) | 同步返回 lastUsed + presets 配置 |

**异步调用模式：**

所有 `@JavascriptInterface` 方法返回 `void`，结果通过 Kotlin→WebView 的 `evaluateJavascript` 回调到 JS。Kotlin 端维护一个 `applyRequestId: AtomicLong` 计数器防止竞态——每次新请求递增，回调时检查是否仍为当前请求，丢弃过期响应。

**save() 执行顺序：**

1. Kotlin 调用 `__tauriEndColorGradingPreview()` — 关闭 Rust preview session + 删除临时文件
2. Kotlin 调用 `__tauriTriggerColorGrading(filePath, lutId, meteringMode, evOffset, false)` — 加入后台处理队列
3. Kotlin 调用 `__tauriSaveColorGradingLastUsed(lutId, meteringMode, evOffset)` — 保存配置
4. 立即 `activity.finish()` — 不等待后台处理完成

**IPC 转发实现：**

所有方法通过 `MainActivity.instance.getWebView().evaluateJavascript()` 调用主 WebView 上的全局函数。对于需要返回值的调用（如 `applyPreview` 返回文件路径），使用以下模式：

```kotlin
// Kotlin 端
fun applyPreview(lutId: String, meteringMode: String, evOffset: Float) {
    val mainActivity = MainActivity.instance ?: return
    mainActivity.runOnUiThread {
        mainActivity.getWebView()?.evaluateJavascript(
            "(async function(){ try { var r = await window.__tauriApplyColorGradingPreview('$lutId','$meteringMode',$evOffset); return r; } catch(e) { return 'error:' + e.message; } })();"
        ) { result ->
            val path = parseJsResult(result)
            if (path != null && !path.startsWith("error:")) {
                previewFilePath = path
                // 通知 WebView 刷新预览
                runOnUiThread { refreshWebViewPreview() }
            } else {
                runOnUiThread { notifyPreviewError(path) }
            }
        }
    }
}
```

### 4.3 shouldInterceptRequest

```kotlin
override fun shouldInterceptRequest(view: WebView, request: WebResourceRequest): WebResourceResponse? {
    if (request.url.scheme == "preview" && request.url.host == "latest") {
        val file = previewFilePath?.let { java.io.File(it) }
        if (file != null && file.exists()) {
            return WebResourceResponse(
                "image/jpeg", null, 200, "OK",
                mapOf("Content-Length" to file.length().toString()),
                file.inputStream()  // Direct FileInputStream, streamed
            )
        }
        return WebResourceResponse("image/jpeg", null, 404, "Not Found", emptyMap(), null)
    }
    return super.shouldInterceptRequest(view, request)
}
```

### 4.4 ImageViewerActivity 修改

`triggerColorGradingForCurrentImage()` 方法的修改：

- **当前行为**：解析 filePath → 获取配置 → `overlayController.showColorGrading()`
- **新行为**：解析 filePath → `startActivity(ColorGradingActivity, filePath)`

`WebViewOverlayController` 的 `showColorGrading` / `dismissColorGrading` 方法保留不动（图库页面仍需要）。`NativeColorGradingBridge` 也保留。

## 5. Frontend (App.tsx) Changes

在 `App.tsx` 的 `useEffect` 中新增全局 JS 函数：

```typescript
w.__tauriBeginColorGradingPreview = async (filePath: string) => {
    await invoke('begin_color_grading_preview', { imagePath: filePath });
};

w.__tauriApplyColorGradingPreview = async (lutId: string, meteringMode: string, evOffset: number) => {
    return await invoke<string>('apply_color_grading_preview', {
        lutId, meteringMode, evOffset,
        enableLensCorrection: true,  // 始终启用镜头校正
    });
};

w.__tauriEndColorGradingPreview = async () => {
    await invoke('end_color_grading_preview');
};

w.__tauriSaveColorGradingLastUsed = (lutId: string, meteringMode: string, evOffset: number) => {
    updateDraft(d => ({
        ...d,
        colorGradingLastUsed: { presetId: lutId, meteringMode, evOffset },
    }));
};
```

## 6. Rust Backend Changes

**无修改。** 现有的 `begin_color_grading_preview`、`apply_color_grading_preview`、`end_color_grading_preview` 命令已满足所有需求。镜头校正始终启用（`enable_lens_correction: true`）。

## 7. HTML Asset

### 7.1 color_grading_preview.html

新建资产文件，放置于 `src-tauri/gen/android/app/src/main/assets/color_grading_preview.html`。

与现有的 `color_grading_dialog.html` 不同，这是一个**全屏应用**而非对话框。

**关键差异：**
- 全屏布局（无半透明 overlay 背景）
- 深色主题（`#111` 背景）
- 预览图区域占据大部分屏幕空间
- 控件固定在底部
- 包含加载状态和错误处理
- 无"同步到自动调色"开关（简化）

**NativeBridge 接口：**

```javascript
// 由 Kotlin NativeColorGradingPreviewBridge 提供
NativeBridge.beginPreview(filePath)      // 开始 RAW 解码
NativeBridge.applyPreview(lutId, meteringMode, evOffset)  // 应用调色
NativeBridge.save(lutId, meteringMode, evOffset)          // 保存并关闭
NativeBridge.cancelPreview()             // 中断/取消
NativeBridge.getConfig()                 // 获取 lastUsed + presets 配置
```

**Kotlin→JS 回调接口：**

```javascript
// 由 HTML JS 提供，Kotlin 通过 evaluateJavascript 调用
window.onPreviewReady()                  // 解码完成，可以 apply
window.onPreviewError(message)           // 解码/应用失败
window.refreshPreview()                  // 刷新预览图（更新 img.src）
```

## 8. Error Handling

| 场景 | 处理 |
|------|------|
| RAW 解码失败 | 显示错误信息 + "重试" / "返回" 按钮 |
| apply 失败 | Toast 提示，保持当前预览图不变 |
| 保存（process_file）失败 | 关闭 Activity，后台任务进度面板显示失败状态 |
| MainActivity 不可用 | Toast "无法连接后端" + 关闭 Activity |
| WebView 加载失败 | 显示原生错误提示 |

## 9. AndroidManifest

需要注册新 Activity：

```xml
<activity
    android:name=".ColorGradingActivity"
    android:theme="@style/Theme.AppCompat.NoActionBar"
    android:screenOrientation="unspecified"
    android:configChanges="orientation|screenSize|keyboardHidden" />
```

## 10. Files Changed

| 文件 | 变更类型 | 说明 |
|------|---------|------|
| `src-tauri/gen/android/.../ColorGradingActivity.kt` | **新增** | 调色 Activity + WebView + Bridge + shouldInterceptRequest |
| `src-tauri/gen/android/.../assets/color_grading_preview.html` | **新增** | 全屏调色 UI |
| `src-tauri/gen/android/.../ImageViewerActivity.kt` | **修改** | `triggerColorGradingForCurrentImage()` 改为启动新 Activity |
| `src-tauri/gen/android/.../AndroidManifest.xml` | **修改** | 注册 ColorGradingActivity |
| `src/App.tsx` | **修改** | 新增 `__tauriBeginColorGradingPreview` 等 4 个全局函数 |
| `src/types/global.ts` | **修改** | 新增全局函数类型声明 |
| `src-tauri/gen/android/app/proguard-rules.pro` | **修改** | 添加 `-keep` 规则（如有新 Bridge 类被 Rust 反射调用） |
