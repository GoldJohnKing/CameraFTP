# 代码质量优化实施计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 消除项目中的历史遗留代码、冗余实现和架构问题，提升代码可维护性

**Architecture:** 分三个阶段执行 - 死代码清理 → 架构重构 → 前端优化，每个阶段独立可验证

**Tech Stack:** Rust (Tauri), TypeScript (React), Zustand

---

## 阶段一：死代码清理（高优先级）

### Task 1.1: 删除未使用的 saf_picker.rs

**Files:**
- Delete: `src-tauri/src/saf_picker.rs`

**Step 1: 验证文件确实未被使用**

```bash
grep -r "saf_picker" src-tauri/src/ --include="*.rs"
# 预期：无输出（或仅在注释中）
```

**Step 2: 删除文件**

```bash
rm src-tauri/src/saf_picker.rs
```

**Step 3: 验证编译**

```bash
cd src-tauri && cargo build
# 预期：编译成功
```

**Step 4: Commit**

```bash
git add -A && git commit -m "chore: remove unused saf_picker.rs module"
```

---

### Task 1.2: 删除 ftp/types.rs 中未使用的类型

**Files:**
- Modify: `src-tauri/src/ftp/types.rs:130-150`

**Step 1: 删除 SessionInfo, TransferInfo, TransferDirection**

删除第 130-150 行：
```rust
// 删除以下代码
/// 会话信息
#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub id: String,
    pub username: String,
    pub connected_at: std::time::Instant,
}

/// 文件传输信息
#[derive(Debug, Clone)]
pub struct TransferInfo {
    pub path: String,
    pub size: u64,
    pub direction: TransferDirection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferDirection {
    Upload,
    Download,
}
```

**Step 2: 验证编译**

```bash
cd src-tauri && cargo build
# 预期：编译成功
```

**Step 3: Commit**

```bash
git add -A && git commit -m "chore: remove unused SessionInfo, TransferInfo, TransferDirection types"
```

---

### Task 1.3: 删除 error.rs 中未使用的 helpers 模块

**Files:**
- Modify: `src-tauri/src/error.rs:290-356`

**Step 1: 删除 helpers 模块**

删除第 290-356 行（整个 `pub mod helpers { ... }` 块）

**Step 2: 验证编译**

```bash
cd src-tauri && cargo build
# 预期：编译成功
```

**Step 3: Commit**

```bash
git add -A && git commit -m "chore: remove unused error::helpers module"
```

---

### Task 1.4: 删除 stats.rs 中已废弃的函数

**Files:**
- Modify: `src-tauri/src/ftp/stats.rs:44-64`

**Step 1: 删除废弃的 get_stats() 和 get_snapshot() 方法**

删除 StatsActor impl 块中的：
```rust
/// 获取当前统计（异步，通过 channel）
#[deprecated(note = "使用 get_stats_direct() 更可靠")]
pub async fn get_stats(&self) -> Option<ServerStats> {
    let (tx, mut rx) = mpsc::channel(1);
    if self.tx.send(StatsCommand::GetStats(tx)).await.is_err() {
        warn!("get_stats: channel send failed");
        return None;
    }
    rx.recv().await
}

/// 获取统计快照（异步，用于快速读取）
#[deprecated(note = "使用 get_stats_direct() 更可靠")]
pub async fn get_snapshot(&self) -> Option<ServerStats> {
    let (tx, mut rx) = mpsc::channel(1);
    if self.tx.send(StatsCommand::GetSnapshot(tx)).await.is_err() {
        warn!("get_snapshot: channel send failed");
        return None;
    }
    rx.recv().await
}
```

**Step 2: 删除相关的 StatsCommand 枚举变体**

从 StatsCommand 枚举中删除：
```rust
GetStats(mpsc::Sender<ServerStats>),
GetSnapshot(mpsc::Sender<ServerStats>),
```

**Step 3: 删除 StatsActorWorker 中对应的处理逻辑**

删除 StatsActorWorker::run() 中的：
```rust
StatsCommand::GetStats(tx) => {
    let stats = self.stats.read().await.clone();
    let _ = tx.send(stats);
}
StatsCommand::GetSnapshot(tx) => {
    if let Ok(stats) = self.stats.try_read() {
        let _ = tx.send(stats.clone());
    } else {
        let _ = tx.send(ServerStats::default());
    }
}
```

**Step 4: 验证编译**

```bash
cd src-tauri && cargo build
# 预期：编译成功
```

**Step 5: Commit**

```bash
git add -A && git commit -m "chore: remove deprecated get_stats() and get_snapshot() methods"
```

---

### Task 1.5: 删除前端未使用的导出

**Files:**
- Modify: `src/utils/format.ts:19-28`
- Modify: `src/types/index.ts:27-33`

**Step 1: 删除 formatBytesAuto 函数**

从 `src/utils/format.ts` 删除第 19-28 行：
```typescript
/**
 * 将字节数格式化为最合适的单位
 * @param bytes 字节数
 * @returns 格式化后的字符串，如 "1.2 GB"、"512.0 MB"
 */
export function formatBytesAuto(bytes: number): string {
  if (bytes === 0) return '0 B';
  
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  const k = 1024;
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  const index = Math.min(i, units.length - 1);
  
  return `${(bytes / Math.pow(k, index)).toFixed(1)} ${units[index]}`;
}
```

**Step 2: 删除 NetworkInterface 类型**

从 `src/types/index.ts` 删除第 27-33 行：
```typescript
export interface NetworkInterface {
  name: string;
  ip: string;
  is_wifi: boolean;
  is_ethernet: boolean;
  is_up: boolean;
}
```

**Step 3: 验证编译**

```bash
bun run build
# 预期：编译成功
```

**Step 4: Commit**

```bash
git add -A && git commit -m "chore: remove unused formatBytesAuto and NetworkInterface exports"
```

---

## 阶段二：架构重构（中优先级）

### Task 2.1: 统一使用 PlatformService trait 回调

**Files:**
- Modify: `src-tauri/src/commands.rs:59-64, 92-97`
- Modify: `src-tauri/src/platform/windows.rs`

**Step 1: 更新 start_server 使用 trait 回调**

将 `commands.rs:59-64`：
```rust
// 更新托盘图标为 idle 状态（服务器运行中，但还没有设备连接）
#[cfg(target_os = "windows")]
{
    if let Err(e) = crate::platform::windows::update_tray_icon(&app, crate::platform::windows::TrayIconState::Idle) {
        warn!(error = %e, "Failed to update tray icon to idle");
    }
}
```

替换为：
```rust
// 使用 PlatformService trait 更新平台状态
crate::platform::get_platform().on_server_started(&app);
```

**Step 2: 更新 stop_server 使用 trait 回调**

将 `commands.rs:92-97`：
```rust
// 更新托盘图标为 stopped 状态（服务器停止）
#[cfg(target_os = "windows")]
{
    if let Err(e) = crate::platform::windows::update_tray_icon(&app, crate::platform::windows::TrayIconState::Stopped) {
        warn!(error = %e, "Failed to update tray icon to stopped");
    }
}
```

替换为：
```rust
// 使用 PlatformService trait 更新平台状态
crate::platform::get_platform().on_server_stopped(&app);
```

**Step 3: 实现 WindowsPlatform 的 on_server_started 和 on_server_stopped**

在 `src-tauri/src/platform/windows.rs` 的 WindowsPlatform impl 块中添加：

```rust
use super::traits::PlatformService;
use tauri::AppHandle;

impl PlatformService for WindowsPlatform {
    // ... 现有方法 ...
    
    fn on_server_started(&self, app: &AppHandle) {
        if let Err(e) = update_tray_icon(app, TrayIconState::Idle) {
            tracing::warn!(error = %e, "Failed to update tray icon to idle");
        }
    }

    fn on_server_stopped(&self, app: &AppHandle) {
        if let Err(e) = update_tray_icon(app, TrayIconState::Stopped) {
            tracing::warn!(error = %e, "Failed to update tray icon to stopped");
        }
    }
}
```

**Step 4: 验证编译**

```bash
cd src-tauri && cargo build
# 预期：编译成功
```

**Step 5: Commit**

```bash
git add -A && git commit -m "refactor: use PlatformService trait for server lifecycle callbacks"
```

---

### Task 2.2: 重构 storage_permission.rs 使用 PlatformService trait

**Files:**
- Modify: `src-tauri/src/storage_permission.rs`

**Step 1: 重构 get_storage_info**

将：
```rust
#[tauri::command]
pub async fn get_storage_info() -> Result<StorageInfo, String> {
    #[cfg(target_os = "android")]
    {
        Ok(android::get_storage_info_impl())
    }
    
    #[cfg(not(target_os = "android"))]
    {
        Ok(StorageInfo {
            display_name: "本地存储".to_string(),
            path: String::new(),
            exists: false,
            writable: false,
            has_all_files_access: false,
        })
    }
}
```

替换为：
```rust
#[tauri::command]
pub async fn get_storage_info() -> Result<StorageInfo, String> {
    Ok(crate::platform::get_platform().get_storage_info())
}
```

**Step 2: 重构 check_permission_status**

将：
```rust
#[tauri::command]
pub async fn check_permission_status() -> Result<PermissionStatus, String> {
    #[cfg(target_os = "android")]
    {
        Ok(android::check_permission_status_impl())
    }
    
    #[cfg(not(target_os = "android"))]
    {
        Ok(PermissionStatus {
            has_all_files_access: true,
            needs_user_action: false,
        })
    }
}
```

替换为：
```rust
#[tauri::command]
pub async fn check_permission_status() -> Result<PermissionStatus, String> {
    Ok(crate::platform::get_platform().check_permission_status())
}
```

**Step 3: 更新 WindowsPlatform 的 get_storage_info 和 check_permission_status**

在 `src-tauri/src/platform/windows.rs` 中确保实现了这些方法：

```rust
fn get_storage_info(&self) -> StorageInfo {
    StorageInfo {
        display_name: "本地存储".to_string(),
        path: String::new(),
        exists: false,
        writable: false,
        has_all_files_access: false,
    }
}

fn check_permission_status(&self) -> PermissionStatus {
    PermissionStatus {
        has_all_files_access: true,
        needs_user_action: false,
    }
}
```

**Step 4: 验证编译**

```bash
cd src-tauri && cargo build
# 预期：编译成功
```

**Step 5: Commit**

```bash
git add -A && git commit -m "refactor: use PlatformService trait in storage_permission commands"
```

---

### Task 2.3: 合并 lib.rs 中的统计推送逻辑

**Files:**
- Modify: `src-tauri/src/lib.rs:148-188`

**Step 1: 删除 lib.rs 中的重复统计推送代码**

删除 `lib.rs` 中第 148-188 行的统计推送定时器：
```rust
// 删除整个这段代码
// 启动统计信息推送定时器（优化：只在有变化时推送）
let app_handle = app.handle().clone();
let state: tauri::State<'_, FtpServerState> = app.state();
let state_clone = state.0.clone();
let platform_ref = platform;

tauri::async_runtime::spawn(async move {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(500));
    let mut last_snapshot: Option<ServerStateSnapshot> = None;

    loop {
        interval.tick().await;

        let server_guard = state_clone.lock().await;
        if let Some(server) = server_guard.as_ref() {
            let snapshot: ServerStateSnapshot = server.get_snapshot().await;

            // 只在服务器运行且状态变化时推送
            if snapshot.is_running {
                let should_emit = match &last_snapshot {
                    None => true,
                    Some(last) => {
                        last.connected_clients != snapshot.connected_clients
                            || last.files_received != snapshot.files_received
                            || last.bytes_received != snapshot.bytes_received
                            || last.last_file != snapshot.last_file
                    }
                };

                if should_emit {
                    let _ = app_handle.emit("stats-update", &snapshot);

                    // 使用 Platform Trait 更新托盘图标状态
                    platform_ref.update_server_state(&app_handle, snapshot.connected_clients as u32);

                    last_snapshot = Some(snapshot);
                }
            }
        }
    }
});
```

**Step 2: 确认 server_factory.rs 的 EventProcessor 已实现完整功能**

验证 `src-tauri/src/ftp/server_factory.rs` 中的 `spawn_event_processor` 已包含：
- stats-update 事件推送
- 托盘图标状态更新（通过 PlatformService trait）

**Step 3: 验证编译和功能**

```bash
cd src-tauri && cargo build
# 预期：编译成功

# 运行应用验证统计功能正常
cargo tauri dev
# 预期：服务器启动后，统计信息正常更新
```

**Step 4: Commit**

```bash
git add -A && git commit -m "refactor: remove duplicate stats polling in lib.rs, use EventProcessor"
```

---

### Task 2.4: 拆分 commands.rs 为多个模块（可选）

**Files:**
- Create: `src-tauri/src/commands/mod.rs`
- Create: `src-tauri/src/commands/server.rs`
- Create: `src-tauri/src/commands/config.rs`
- Create: `src-tauri/src/commands/network.rs`
- Create: `src-tauri/src/commands/platform.rs`
- Delete: `src-tauri/src/commands.rs`

**注意：这是一个较大的重构，建议单独分支进行**

**Step 1: 创建 commands 目录结构**

```bash
mkdir -p src-tauri/src/commands
```

**Step 2: 创建 commands/server.rs**

```rust
use tauri::{command, AppHandle, Emitter, Manager, State};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{error, info, instrument, warn};
use ts_rs::TS;

use crate::error::AppError;
use crate::ftp::types::ServerStateSnapshot;
use crate::ftp::FtpServerHandle;

/// FTP 服务器状态
pub struct FtpServerState(pub Arc<Mutex<Option<FtpServerHandle>>>);

/// 服务器信息
#[derive(Debug, Clone, serde::Serialize, TS)]
#[ts(export)]
pub struct ServerInfo {
    pub is_running: bool,
    pub ip: String,
    pub port: u16,
    pub url: String,
    pub username: String,
    pub password_info: String,
}

#[command]
#[instrument(skip(state, app))]
pub async fn start_server(
    state: State<'_, FtpServerState>,
    app: AppHandle,
) -> Result<ServerInfo, AppError> {
    // ... 实现 ...
}

#[command]
#[instrument(skip(state, app))]
pub async fn stop_server(
    state: State<'_, FtpServerState>,
    app: AppHandle,
) -> Result<(), AppError> {
    // ... 实现 ...
}

#[command]
#[instrument(skip(state))]
pub async fn get_server_status(
    state: State<'_, FtpServerState>,
) -> Result<Option<ServerStateSnapshot>, AppError> {
    // ... 实现 ...
}
```

**Step 3-5: 类似创建其他模块...**

**注意：完整实现较长，建议在实际执行时详细展开**

---

## 阶段三：前端优化（中优先级）

### Task 3.1: 提取共享 UI 组件 - LoadingButton

**Files:**
- Create: `src/components/ui/LoadingButton.tsx`

**Step 1: 创建 LoadingButton 组件**

```typescript
// src/components/ui/LoadingButton.tsx
import { Loader2 } from 'lucide-react';
import { ButtonHTMLAttributes, ReactNode } from 'react';

interface LoadingButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  isLoading: boolean;
  loadingText?: string;
  children: ReactNode;
  variant?: 'primary' | 'secondary' | 'danger';
}

export function LoadingButton({
  isLoading,
  loadingText,
  children,
  variant = 'primary',
  className = '',
  disabled,
  ...props
}: LoadingButtonProps) {
  const baseStyles = 'w-full py-3 rounded-lg font-medium transition-all duration-200 flex items-center justify-center gap-2';
  
  const variantStyles = {
    primary: 'bg-blue-500 hover:bg-blue-600 text-white disabled:bg-gray-300',
    secondary: 'bg-gray-100 hover:bg-gray-200 text-gray-800 disabled:bg-gray-100',
    danger: 'bg-red-500 hover:bg-red-600 text-white disabled:bg-gray-300',
  };

  return (
    <button
      className={`${baseStyles} ${variantStyles[variant]} ${className}`}
      disabled={disabled || isLoading}
      {...props}
    >
      {isLoading && <Loader2 className="w-5 h-5 animate-spin" />}
      {isLoading ? (loadingText || '处理中...') : children}
    </button>
  );
}
```

**Step 2: 更新 ServerCard.tsx 使用 LoadingButton**

```typescript
// 在 ServerCard.tsx 顶部添加
import { LoadingButton } from './ui/LoadingButton';

// 替换按钮部分
<LoadingButton
  isLoading={isLoading}
  loadingText={isRunning ? '停止中...' : '启动中...'}
  onClick={isRunning ? handleStop : handleStart}
  variant={isRunning ? 'danger' : 'primary'}
>
  {isRunning ? '停止服务器' : '启动服务器'}
</LoadingButton>
```

**Step 3: 验证编译**

```bash
bun run build
# 预期：编译成功
```

**Step 4: Commit**

```bash
git add -A && git commit -m "refactor: extract LoadingButton component"
```

---

### Task 3.2: 提取共享 UI 组件 - ErrorMessage

**Files:**
- Create: `src/components/ui/ErrorMessage.tsx`

**Step 1: 创建 ErrorMessage 组件**

```typescript
// src/components/ui/ErrorMessage.tsx
import { AlertCircle } from 'lucide-react';

interface ErrorMessageProps {
  message: string | null;
  onDismiss?: () => void;
  variant?: 'inline' | 'card';
}

export function ErrorMessage({ message, onDismiss, variant = 'inline' }: ErrorMessageProps) {
  if (!message) return null;

  if (variant === 'card') {
    return (
      <div className="p-3 bg-red-50 border border-red-200 rounded-lg flex items-start gap-2">
        <AlertCircle className="w-5 h-5 text-red-500 flex-shrink-0 mt-0.5" />
        <div className="flex-1">
          <p className="text-sm text-red-600">{message}</p>
        </div>
        {onDismiss && (
          <button
            onClick={onDismiss}
            className="text-red-400 hover:text-red-600 text-sm"
          >
            ✕
          </button>
        )}
      </div>
    );
  }

  return (
    <p className="mt-3 text-sm text-red-600 text-center flex items-center justify-center gap-1">
      <AlertCircle className="w-4 h-4" />
      {message}
    </p>
  );
}
```

**Step 2: 更新 ServerCard.tsx 和 ConfigCard.tsx 使用 ErrorMessage**

**Step 3: 验证编译并 Commit**

---

### Task 3.3: 提取共享 UI 组件 - ToggleSwitch

**Files:**
- Create: `src/components/ui/ToggleSwitch.tsx`

**Step 1: 创建 ToggleSwitch 组件**

```typescript
// src/components/ui/ToggleSwitch.tsx
interface ToggleSwitchProps {
  enabled: boolean;
  onChange: (enabled: boolean) => void;
  label: string;
  description?: string;
  disabled?: boolean;
}

export function ToggleSwitch({ enabled, onChange, label, description, disabled }: ToggleSwitchProps) {
  return (
    <div className="flex items-center justify-between">
      <div>
        <span className="text-sm font-medium text-gray-700">{label}</span>
        {description && (
          <p className="text-xs text-gray-500 mt-0.5">{description}</p>
        )}
      </div>
      <button
        type="button"
        onClick={() => !disabled && onChange(!enabled)}
        disabled={disabled}
        className={`
          relative inline-flex h-6 w-11 items-center rounded-full
          transition-colors duration-200 ease-in-out
          ${enabled ? 'bg-blue-500' : 'bg-gray-300'}
          ${disabled ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer'}
        `}
      >
        <span
          className={`
            inline-block h-4 w-4 transform rounded-full bg-white
            transition-transform duration-200 ease-in-out
            ${enabled ? 'translate-x-6' : 'translate-x-1'}
          `}
        />
      </button>
    </div>
  );
}
```

**Step 2: 更新 ConfigCard.tsx 使用 ToggleSwitch**

**Step 3: 验证编译并 Commit**

---

### Task 3.4: 移除 serverStore 中的冗余状态

**Files:**
- Modify: `src/stores/serverStore.ts`

**Step 1: 移除 isRunning 状态，使用派生值**

将：
```typescript
interface ServerState {
  isRunning: boolean;
  serverInfo: ServerInfo | null;
  // ...
}
```

改为：
```typescript
interface ServerState {
  serverInfo: ServerInfo | null;
  // ... 移除 isRunning
  
  // 添加 getter
  isRunning: () => boolean;
}
```

实现：
```typescript
export const useServerStore = create<ServerState>((set, get) => ({
  serverInfo: null,
  
  // 派生状态
  isRunning: () => get().serverInfo !== null && get().serverInfo.is_running,
  
  // ... 其他方法
}));
```

**注意：这可能需要更新所有使用 `isRunning` 的组件，工作量较大**

---

## 阶段四：类型安全增强（高优先级）

### Task 4.1: 启用 ts_rs 自动生成前端类型

**Files:**
- Modify: `src-tauri/src/config.rs`
- Modify: `src-tauri/src/network.rs`
- Modify: `src-tauri/src/platform/types.rs`

**Step 1: 为 AppConfig 添加 ts_rs 导出**

在 `src-tauri/src/config.rs` 中：
```rust
use ts_rs::TS;

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AppConfig {
    pub save_path: PathBuf,
    pub auto_open: bool,
    pub auto_open_program: Option<String>,
    pub port: u16,
    pub auto_select_port: bool,
    pub file_extensions: Vec<String>,
}
```

**Step 2: 为 NetworkInterface 添加 ts_rs 导出**

在 `src-tauri/src/network.rs` 中：
```rust
use ts_rs::TS;

#[derive(Debug, Clone, serde::Serialize, TS)]
#[ts(export)]
pub struct NetworkInterface {
    pub name: String,
    pub ip: String,
    pub is_wifi: bool,
    pub is_ethernet: bool,
    pub is_up: bool,
}
```

**Step 3: 为 platform types 添加 ts_rs 导出**

在 `src-tauri/src/platform/types.rs` 中：
```rust
use ts_rs::TS;

#[derive(Debug, Clone, serde::Serialize, TS)]
#[ts(export)]
pub struct StorageInfo {
    pub display_name: String,
    pub path: String,
    pub exists: bool,
    pub writable: bool,
    pub has_all_files_access: bool,
}

#[derive(Debug, Clone, serde::Serialize, TS)]
#[ts(export)]
pub struct PermissionStatus {
    pub has_all_files_access: bool,
    pub needs_user_action: bool,
}

#[derive(Debug, Clone, serde::Serialize, TS)]
#[ts(export)]
pub struct ServerStartCheckResult {
    pub can_start: bool,
    pub reason: Option<String>,
    pub storage_info: Option<StorageInfo>,
}
```

**Step 4: 配置构建脚本自动生成类型**

在 `src-tauri/build.rs` 中添加：
```rust
fn main() {
    // 现有的构建逻辑...
    
    // 生成 TypeScript 类型
    tauri_build::build();
}
```

**Step 5: 更新前端使用生成的类型**

```typescript
// src/types/index.ts
// 从生成的类型导入，而不是手动定义
export type { ServerInfo } from './generated/ServerInfo';
export type { ServerStateSnapshot } from './generated/ServerStateSnapshot';
export type { AppConfig } from './generated/AppConfig';
export type { NetworkInterface } from './generated/NetworkInterface';
export type { StorageInfo, PermissionStatus, ServerStartCheckResult } from './generated/platform';
```

**Step 6: 验证编译**

```bash
cd src-tauri && cargo build
bun run build
# 预期：编译成功
```

**Step 7: Commit**

```bash
git add -A && git commit -m "feat: enable ts_rs auto-generation for TypeScript types"
```

---

## 验证清单

完成所有任务后，执行以下验证：

```bash
# 1. Rust 编译检查
cd src-tauri && cargo build && cargo clippy

# 2. 前端编译检查
bun run build

# 3. 运行应用验证功能
cargo tauri dev
# - 测试服务器启动/停止
# - 验证统计信息更新
# - 检查托盘图标状态（Windows）
# - 验证 Android 权限流程

# 4. 确认无死代码警告
cargo build 2>&1 | grep -i "dead_code"
# 预期：无输出
```

---

## 执行顺序建议

1. **阶段一（Task 1.1-1.5）**：死代码清理 - 风险低，收益明确
2. **阶段四（Task 4.1）**：类型安全增强 - 消除类型同步风险
3. **阶段二（Task 2.1-2.3）**：架构重构 - 提升代码一致性
4. **阶段三（Task 3.1-3.4）**：前端优化 - 可选，按需执行

**预计总工作量**：4-6 小时

---

## 风险评估

| 任务 | 风险等级 | 说明 |
|------|---------|------|
| Task 1.x 死代码删除 | 低 | 仅删除未使用的代码 |
| Task 2.1-2.2 trait 统一 | 低 | 重构到现有抽象 |
| Task 2.3 合并统计逻辑 | 中 | 需要验证功能完整性 |
| Task 2.4 拆分 commands | 高 | 大规模重构，建议单独分支 |
| Task 3.x 前端组件提取 | 低 | 纯重构，不改变行为 |
| Task 4.1 ts_rs 类型生成 | 中 | 需要更新导入路径 |

