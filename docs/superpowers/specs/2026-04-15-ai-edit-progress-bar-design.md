# AI 修图进度条设计

## 目标

移除所有 AI 修图相关的 Toast 通知，改为显示带动画的进度条 + "第 X 张 / 共 N 张" 文字。

**核心原则：只要修图队列中有任务（无论手动还是 FTP 自动触发），就要显示进度条。**

## 涉及场景

| 场景 | 平台 | 当前行为 | 目标行为 |
|------|------|---------|---------|
| PreviewWindow 单图 | Windows | 按钮变蓝 + "AI修图中..." | 工具栏上方显示进度条 + "第1张/共1张" |
| Gallery 批量修图 | WebView (Win/Android) | toast + fire-and-forget | 底部浮层进度条 |
| FTP 自动修图 | 全平台 | 完全静默，无任何反馈 | 进度条显示自动修图进度 |
| Native 图片查看 | Android | Toast "正在修图…"/"修图完成"/"修图失败" | 进度条 UI 替代 Toast |

## 架构：后端事件驱动

由于 FTP 自动修图完全在后端发起（`AiEditService.on_file_uploaded()`），前端无法通过 `invoke` 追踪。需要改为 **后端推送进度事件，前端监听**。

### 进度事件

**事件名**: `ai-edit-progress`

**事件 payload（使用 ts-rs 生成 TypeScript 类型）：**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AiEditProgressEvent {
    Progress {
        current: u32,
        total: u32,
        #[serde(rename = "fileName")]
        #[ts(rename = "fileName")]
        file_name: String,
        #[serde(rename = "failedCount")]
        #[ts(rename = "failedCount")]
        failed_count: u32,
    },
    Completed {
        current: u32,
        total: u32,
        #[serde(rename = "fileName")]
        #[ts(rename = "fileName")]
        file_name: String,
        #[serde(rename = "failedCount")]
        #[ts(rename = "failedCount")]
        failed_count: u32,
    },
    /// Single task failure — queue continues
    Failed {
        current: u32,
        total: u32,
        #[serde(rename = "fileName")]
        #[ts(rename = "fileName")]
        file_name: String,
        error: String,
        #[serde(rename = "failedCount")]
        #[ts(rename = "failedCount")]
        failed_count: u32,
    },
    Done {
        total: u32,
        #[serde(rename = "failedCount")]
        #[ts(rename = "failedCount")]
        failed_count: u32,
        #[serde(rename = "failedFiles")]
        #[ts(rename = "failedFiles")]
        failed_files: Vec<String>,
    },
}
```

### 后端改动（Rust）

**`service.rs` — `worker_loop`**：

1. 维护一个 `queue_depth: Arc<AtomicU32>` 计数器，记录当前队列中的总任务数（manual + auto）
2. 每次从 channel 取出任务时，emit `Progress { current, total, file_name }`
3. 任务完成后 emit `Completed` 或 `Failed`
4. 队列清空时 emit `Done`
5. 通过 `app_handle.emit("ai-edit-progress", &event)` 推送

**`service.rs` — `on_file_uploaded` / `edit_single`**：

- 入队时 `queue_depth.fetch_add(1)` + emit `Progress` 更新 total
- 新增 `pub fn queue_len(&self) -> u32` 方法供前端查询当前队列深度

**新增 Tauri 命令** — `get_ai_edit_queue_depth`：

```rust
#[command]
pub async fn get_ai_edit_queue_depth(ai_edit: State<'_, AiEditService>) -> Result<u32, AppError> {
    Ok(ai_edit.queue_len())
}
```

### 前端监听

**`useAiEditProgress` hook**：

```typescript
import { listen } from '@tauri-apps/api/event';

// 监听后端事件，更新 zustand store
listen<AiEditProgressEvent>('ai-edit-progress', (event) => {
  // 更新进度状态
});
```

- 组件 mount 时开始监听
- 收到 `Progress` → 显示进度条
- 收到 `Done` → 隐藏进度条
- 收到 `Failed` → 显示错误状态
- unmount 时自动清理 listener

## 手动修图：追加队列

手动触发修图时，前端不再逐个 `invoke`，而是调用新命令 `enqueue_ai_edit`：

**新增 Tauri 命令**：
```rust
#[command]
pub async fn enqueue_ai_edit(
    ai_edit: State<'_, AiEditService>,
    file_paths: Vec<String>,
    prompt: Option<String>,
) -> Result<(), AppError> {
    for path in file_paths {
        ai_edit.enqueue_manual(PathBuf::from(path), prompt.clone()).await?;
    }
    Ok(())
}
```

后端 `AiEditService` 新增 `enqueue_manual` 方法（复用现有 `edit_single` 的逻辑但支持批量入队，失败不终止）。

**前端**：
- `handleAiEditPromptConfirm` 调用 `invoke('enqueue_ai_edit', { filePaths, prompt })`
- 不再需要前端顺序循环 — 后端队列本身保证顺序执行
- 进度完全由后端事件驱动

### 追加队列行为

当修图进行中用户再次点击"修图"：
1. 前端调用 `invoke('enqueue_ai_edit', { filePaths: [...newFiles], prompt })`
2. 后端将新任务追加到 manual channel
3. worker_loop 取到新任务时 emit `Progress` 更新 total
4. 进度条自动刷新显示新的 "第X张/共N张"

### 失败处理行为

- **所有任务（手动和自动）失败都不终止队列**，继续处理下一张
- 后端在 `Done` 事件中携带 `failed_count` 和 `failed_files` 汇总信息
- 前端收到 `Failed` 事件时累加 `failedCount`，进度文字变为 "第3张/共5张 (失败1张)"
- 队列全部完成后（`Done`）：
  - `failed_count === 0` → 进度条正常消失
  - `failed_count > 0` → 进度条变为红色，显示失败摘要（如 "修图完成，2张失败"），用户点击关闭后消失

## 统一进度条组件

### AiEditProgressBar（React）

所有 WebView 场景共用同一个组件，父容器决定定位方式。

**视觉设计：**
```
处理中：                                       
┌─────────────────────────────────────────┐
│ ████████████░░░░░░░░  第2张/共5张    ✕  │
└─────────────────────────────────────────┘

处理中（有失败）：                                    
┌──────────────────────────────────────────────┐
│ ████████████░░░░░░░░  第3张/共5张 (失败1张) ✕│
└──────────────────────────────────────────────┘

队列完成（有失败）：                                    
┌──────────────────────────────────────────────┐
│ 修图完成，2张失败                          ✕  │  ← 红色背景
└──────────────────────────────────────────────┘
```

- 半透明暗色背景（`bg-black/70 backdrop-blur-sm`），圆角 `rounded-xl`
- 蓝色渐变动画条（`bg-gradient-to-r from-blue-500 to-blue-400`）
- 文字白色，格式 "第X张/共N张"
- 右侧 ✕ 取消按钮（仅手动修图时显示；纯自动修图时隐藏取消按钮）
- 进入/退出均有过渡动画（`transition-all duration-300`）
- 进度条有 shimmer 动画（CSS `@keyframes shimmer`）

**Props：**
```typescript
interface AiEditProgressBarProps {
  position: 'absolute' | 'fixed';
}
```

**定位差异：**
- `position='absolute'`：PreviewWindow 中，定位在工具栏正上方
- `position='fixed'`：Gallery 主界面中，固定在页面底部（`bottom-4`）

### Android Native 进度条

在 `ImageViewerActivity` 的布局 XML 中，`bottom_bar` 上方添加进度条容器：
- 半透明暗色背景，圆角
- `ProgressBar` (horizontal style) + `TextView`（"第X张/共N张"）
- 通过代码控制可见性（`View.VISIBLE` / `View.GONE`）

Android Native 进度条通过 JS bridge 与 WebView 同步：
- WebView 中的 `useAiEditProgress` 监听后端事件
- 进度变化时通过 `window.ImageViewerAndroid?.updateAiEditProgress(current, total)` 通知 Native 层
- Native 层更新进度条 UI

## 状态管理：useAiEditProgress

**文件**: `src/hooks/useAiEditProgress.ts`（新建）

```typescript
interface AiEditProgress {
  isEditing: boolean;
  isDone: boolean;       // 队列是否全部完成（含失败）
  current: number;       // 当前第几张（1-based）
  total: number;         // 总张数
  currentFileName: string;
  failedCount: number;   // 累计失败张数
  failedFiles: string[]; // 失败文件名列表（队列完成后填充）
  isManual: boolean;     // 是否为手动触发的修图（决定是否显示取消按钮）
}
```

### API

```typescript
function useAiEditProgress(): AiEditProgress;

/** 手动入队修图任务 */
function enqueueAiEdit(files: string[], prompt: string, shouldSave: boolean): Promise<void>;

/** 取消当前修图（中断后续任务） */
function cancelAiEdit(): Promise<void>;
```

### 核心逻辑

1. 组件 mount 时 `listen('ai-edit-progress', handler)` 注册监听
2. 收到 `Progress` → `isEditing = true`，更新 `current`, `total`, `currentFileName`
3. 收到 `Completed` → 更新计数（下一张开始时会有新的 `Progress` 事件）
4. 收到 `Failed` → 累加 `failedCount`，进度文字变为 "第X张/共N张 (失败M张)"
5. 收到 `Done` → `isEditing = false`, `isDone = true`，填充 `failedFiles`
   - `failedCount === 0` → 进度条自动 fadeout
   - `failedCount > 0` → 进度条变红，显示 "修图完成，M张失败"，等待用户关闭
6. `enqueueAiEdit` → `invoke('enqueue_ai_edit', { filePaths, prompt })`
7. `cancelAiEdit` → `invoke('cancel_ai_edit')`

### Android JS Bridge 回调

Native ImageViewerActivity 触发修图时：
1. 调用 `window.__tauriTriggerAiEditWithPrompt(filePath, prompt, shouldSave)`
2. 前端调用 `invoke('enqueue_ai_edit', { filePaths: [filePath], prompt })`
3. 后端事件驱动进度更新
4. 前端 hook 收到 `Done` 时，调用 `window.ImageViewerAndroid?.onAiEditComplete(failedCount === 0, summaryMessage)`

进度同步到 Native 层：
- 每次 `Progress` 事件时调用 `window.ImageViewerAndroid?.updateAiEditProgress?.(current, total, failedCount)`

## 文件改动清单

### 后端（Rust）

| 文件 | 操作 | 说明 |
|------|------|------|
| `src-tauri/src/ai_edit/progress.rs` | 新建 | `AiEditProgressEvent` 枚举 + ts-rs 导出 |
| `src-tauri/src/ai_edit/service.rs` | 改 | worker_loop 中 emit 进度事件；新增 `enqueue_manual`、`queue_len`、`cancel` 方法 |
| `src-tauri/src/commands/ai_edit.rs` | 改 | 新增 `enqueue_ai_edit`、`cancel_ai_edit`、`get_ai_edit_queue_depth` 命令 |
| `src-tauri/src/lib.rs` | 改 | 注册新命令 |
| `src-tauri/src/ai_edit/mod.rs` | 改 | 导出 progress 模块 |

### 前端（TypeScript/React）

| 文件 | 操作 | 说明 |
|------|------|------|
| `src/hooks/useAiEditProgress.ts` | 新建 | 进度状态管理 hook，监听后端事件 |
| `src/components/AiEditProgressBar.tsx` | 新建 | 可复用进度条 UI 组件 |
| `src/components/PreviewWindow.tsx` | 改 | 移除 `aiEditing` 状态；使用全局进度 hook；渲染进度条 |
| `src/hooks/useGallerySelection.ts` | 改 | 移除 `toast.success`；改用 `enqueueAiEdit` |
| `src/App.tsx` | 改 | JS bridge 适配；渲染全局进度条组件 |
| `src/types/index.ts` | 改 | 导出新类型 |

### Android Native

| 文件 | 操作 | 说明 |
|------|------|------|
| `ImageViewerActivity.kt` | 改 | 移除所有 Toast；添加进度条 UI + `updateAiEditProgress` JS bridge |
| `activity_image_viewer.xml` (portrait) | 改 | 添加进度条布局 |
| `activity_image_viewer.xml` (landscape) | 改 | 同上 |
| `drawable/progress_bar_bg.xml` | 新建 | 进度条背景 drawable |
| `drawable/progress_bar_fill.xml` | 新建 | 进度条填充 drawable（蓝色渐变） |

## 错误处理

- 单张失败（手动或自动）→ **不终止队列**，继续下一张，进度文字追加 "(失败N张)"
- 队列全部完成且无失败 → 进度条自动 fadeout
- 队列全部完成且有失败 → 进度条变红（`bg-red-500/80`），显示 "修图完成，N张失败"，用户点击关闭后消失
- 取消操作（✕） → 调用 `cancel_ai_edit`，清空队列，进度条消失

## 动画

- 进度条进入：`translateY(20px) → translateY(0)` + `opacity 0 → 1`
- 进度条退出：反向动画
- 进度填充条：CSS shimmer 动画（光带从左到右扫过）
- 文字变化：`transition-all` 平滑过渡

## 保留不变

- `trigger_ai_edit` 命令保留（向后兼容，Android JS bridge 可继续使用）
- FTP 上传触发自动修图的入口不变（`on_file_uploaded`）
- 后端 dual-channel 优先级架构不变（manual 优先于 auto）
