# 架构重构实施计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 优化跨平台架构，消除冗余代码，统一配置，实现平台抽象层。

**Architecture:** 采用 Trait 抽象层统一平台接口，合并重复类型定义，提取共享工具函数，统一构建入口。

**Tech Stack:** Rust (Tauri v2), TypeScript/React, Zustand

---

## Phase 1: P0 紧急修复（立即执行）

### Task 1: 删除死代码 StorageSettings.tsx

**Files:**
- Delete: `src/components/StorageSettings.tsx`

**Step 1: 确认文件未被引用**

Run: `grep -r "StorageSettings" src/ --include="*.tsx" --include="*.ts"`
Expected: 仅出现 `src/components/StorageSettings.tsx` 自身

**Step 2: 删除文件**

```bash
rm src/components/StorageSettings.tsx
```

**Step 3: 验证构建通过**

Run: `bun run build`
Expected: 构建成功，无错误

**Step 4: 提交**

```bash
git add -A
git commit -m "refactor: remove unused StorageSettings.tsx component"
```

---

### Task 2: 修复 minSdkVersion 配置冲突

**Files:**
- Modify: `src-tauri/mobile.toml:5`
- Modify: `src-tauri/tauri.conf.json:37-39`

**Step 1: 更新 mobile.toml**

将 `src-tauri/mobile.toml` 第 5 行：

```toml
# 修改前
minSdkVersion = 24

# 修改后
minSdkVersion = 30
```

**Step 2: 移除 tauri.conf.json 中的重复配置**

删除 `src-tauri/tauri.conf.json` 第 37-39 行的 `android` 配置块：

```json
// 删除这部分：
    "android": {
      "minSdkVersion": 30
    }
```

最终 `bundle` 部分应为：

```json
"bundle": {
  "active": true,
  "targets": "all",
  "icon": [
    "icons/32x32.png",
    "icons/128x128.png",
    "icons/icon.ico",
    "icons/icon.png"
  ]
}
```

**Step 3: 验证 Android 构建配置**

Run: `grep -r "minSdk" src-tauri/`
Expected: 仅在 `mobile.toml` 中出现 `minSdkVersion = 30`

**Step 4: 提交**

```bash
git add src-tauri/mobile.toml src-tauri/tauri.conf.json
git commit -m "fix: unify minSdkVersion to 30, remove duplicate config"
```

---

## Phase 2: P1 代码质量改进

### Task 3: 提取共享工具函数 formatBytes

**Files:**
- Create: `src/utils/format.ts`
- Modify: `src/components/StatsCard.tsx:7-11`

**Step 1: 创建工具文件**

```typescript
// src/utils/format.ts

/**
 * 将字节数格式化为可读的 MB 字符串
 * @param bytes 字节数
 * @returns 格式化后的字符串，如 "12.5 MB"
 */
export function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 MB';
  const mb = bytes / (1024 * 1024);
  return `${mb.toFixed(1)} MB`;
}

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

**Step 2: 更新 StatsCard.tsx**

```typescript
// src/components/StatsCard.tsx
// 修改导入部分
import { Camera, Image, HardDrive, Clock } from 'lucide-react';
import { useServerStore } from '../stores/serverStore';
import { formatBytes } from '../utils/format';  // 新增导入

// 删除本地 formatBytes 函数（第 7-11 行）
// export function StatsCard() {
//   const { stats } = useServerStore();
//   
//   // 删除这部分：
//   // const formatBytes = (bytes: number): string => {
//   //   if (bytes === 0) return '0 MB';
//   //   const mb = bytes / (1024 * 1024);
//   //   return `${mb.toFixed(1)} MB`;
//   // };
//   ...
// }
```

**Step 3: 验证构建**

Run: `bun run build`
Expected: 构建成功

**Step 4: 提交**

```bash
git add src/utils/format.ts src/components/StatsCard.tsx
git commit -m "refactor: extract formatBytes to shared utility"
```

---

### Task 4: 合并双重错误类型层级

**Files:**
- Modify: `src-tauri/src/error.rs:254-276` (删除 From<FtpError> 实现)
- Modify: `src-tauri/src/ftp/error.rs` (简化为仅保留 FTP 特有错误)
- Modify: `src-tauri/src/ftp/mod.rs` (更新导出)

**Step 1: 分析当前错误类型**

当前状态：
- `AppError` 有 `ServerAlreadyRunning`, `ServerNotRunning`, `FtpServerError`
- `FtpError` 有 `ServerAlreadyRunning`, `ServerNotRunning`, `BindFailed`, `InvalidConfiguration`, `StorageBackendError`, `Io`, `Other`
- 存在 `impl From<FtpError> for AppError` 进行转换

目标：
- `FtpError` 仅保留 FTP 特有的错误：`BindFailed`
- 其他错误直接使用 `AppError`
- 更新所有使用点

**Step 2: 简化 FtpError**

```rust
// src-tauri/src/ftp/error.rs
use std::fmt;

/// FTP模块专用错误类型（仅包含FTP特有错误）
#[derive(Debug)]
pub enum FtpError {
    /// 端口绑定失败
    BindFailed { addr: String, source: std::io::Error },
    /// 其他IO错误
    Io(std::io::Error),
}

impl fmt::Display for FtpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BindFailed { addr, source } => {
                write!(f, "绑定地址失败 {}: {}", addr, source)
            }
            Self::Io(err) => write!(f, "IO错误: {}", err),
        }
    }
}

impl std::error::Error for FtpError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::BindFailed { source, .. } => Some(source),
            Self::Io(err) => Some(err),
        }
    }
}

impl From<std::io::Error> for FtpError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

/// FTP操作结果类型
pub type FtpResult<T> = Result<T, FtpError>;
```

**Step 3: 更新 From<FtpError> for AppError**

```rust
// src-tauri/src/error.rs 第 254-276 行替换为：

impl From<crate::ftp::FtpError> for AppError {
    fn from(err: crate::ftp::FtpError) -> Self {
        match err {
            crate::ftp::FtpError::BindFailed { addr, source } => {
                AppError::NetworkError(format!("Failed to bind to {}: {}", addr, source))
            }
            crate::ftp::FtpError::Io(io_err) => AppError::from(io_err),
        }
    }
}
```

**Step 4: 更新 FTP 模块中的错误使用**

需要在以下文件中将 `FtpError::ServerAlreadyRunning` 等替换为 `AppError`:
- `src-tauri/src/ftp/server.rs`
- `src-tauri/src/ftp/server_factory.rs`

关键更改：
```rust
// 将 FtpError::ServerAlreadyRunning 替换为
AppError::ServerAlreadyRunning

// 将 FtpError::ServerNotRunning 替换为
AppError::ServerNotRunning

// 将 FtpError::InvalidConfiguration 替换为
AppError::ConfigError

// 将 FtpError::StorageBackendError 替换为
AppError::Io

// 将 FtpError::Other 替换为
AppError::Other
```

**Step 5: 验证编译**

Run: `cd src-tauri && cargo build`
Expected: 编译成功

**Step 6: 提交**

```bash
git add src-tauri/src/error.rs src-tauri/src/ftp/error.rs src-tauri/src/ftp/server.rs src-tauri/src/ftp/server_factory.rs src-tauri/src/ftp/mod.rs
git commit -m "refactor: merge duplicate error types, simplify FtpError to FTP-specific only"
```

---

### Task 5: 统一 StorageInfo 和 PermissionStatus 类型定义

**Files:**
- Create: `src-tauri/src/platform/types.rs`
- Modify: `src-tauri/src/platform/mod.rs`
- Modify: `src-tauri/src/platform/android.rs:15-36` (移除类型定义)
- Modify: `src-tauri/src/storage_permission.rs:14-29` (移除重复定义)

**Step 1: 创建共享类型文件**

```rust
// src-tauri/src/platform/types.rs
use serde::Serialize;

/// 存储路径信息
#[derive(Debug, Clone, Serialize)]
pub struct StorageInfo {
    /// 显示名称
    pub display_name: String,
    /// 完整文件系统路径
    pub path: String,
    /// 路径是否存在
    pub exists: bool,
    /// 是否可写
    pub writable: bool,
    /// 是否有所有文件访问权限
    pub has_all_files_access: bool,
}

/// 权限状态
#[derive(Debug, Clone, Serialize)]
pub struct PermissionStatus {
    /// 是否有"所有文件访问权限"
    pub has_all_files_access: bool,
    /// 是否需要用户操作
    pub needs_user_action: bool,
}

/// 服务器启动检查结果
#[derive(Debug, Clone, Serialize)]
pub struct ServerStartCheckResult {
    pub can_start: bool,
    pub reason: Option<String>,
    pub storage_info: Option<StorageInfo>,
}
```

**Step 2: 更新 platform/mod.rs**

```rust
// src-tauri/src/platform/mod.rs
mod types;
pub use types::{StorageInfo, PermissionStatus, ServerStartCheckResult};

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "android")]
pub mod android;
```

**Step 3: 更新 android.rs**

移除 `StorageInfo` 和 `PermissionStatus` 定义（第 15-36 行），改为使用导出：

```rust
// src-tauri/src/platform/android.rs
use super::types::{StorageInfo, PermissionStatus};
// ... 其余代码保持不变
```

**Step 4: 更新 storage_permission.rs**

移除重复的类型定义，改为使用 platform 模块导出：

```rust
// src-tauri/src/storage_permission.rs
use tauri::AppHandle;

#[cfg(target_os = "android")]
use tracing::info;

// 使用统一的类型
pub use crate::platform::{StorageInfo, PermissionStatus, ServerStartCheckResult};

#[cfg(target_os = "android")]
use crate::platform::android;

// ... 其余代码保持不变，移除第 14-29 行的重复类型定义
```

**Step 5: 验证编译**

Run: `cd src-tauri && cargo build`
Expected: 编译成功

**Step 6: 提交**

```bash
git add src-tauri/src/platform/types.rs src-tauri/src/platform/mod.rs src-tauri/src/platform/android.rs src-tauri/src/storage_permission.rs
git commit -m "refactor: unify StorageInfo/PermissionStatus types to platform module"
```

---

## Phase 3: P2 架构优化

### Task 6: 实现 Platform Trait 抽象层

**Files:**
- Create: `src-tauri/src/platform/traits.rs`
- Modify: `src-tauri/src/platform/mod.rs`
- Modify: `src-tauri/src/platform/windows.rs`
- Modify: `src-tauri/src/platform/android.rs`
- Modify: `src-tauri/src/lib.rs`

**Step 1: 创建 Platform Trait**

```rust
// src-tauri/src/platform/traits.rs
use tauri::AppHandle;
use super::types::{StorageInfo, PermissionStatus};

/// 平台服务接口
/// 定义各平台需要实现的统一接口
pub trait PlatformService: Send + Sync {
    /// 获取平台名称
    fn name(&self) -> &'static str;
    
    /// 初始化平台特定功能（托盘、权限等）
    fn setup(&self, app: &AppHandle) -> Result<(), Box<dyn std::error::Error>>;
    
    /// 获取存储路径信息
    fn get_storage_info(&self) -> StorageInfo;
    
    /// 检查权限状态
    fn check_permission_status(&self) -> PermissionStatus;
    
    /// 确保存储就绪
    fn ensure_storage_ready(&self) -> Result<String, String>;
    
    /// 服务器启动时的回调
    fn on_server_started(&self, _app: &AppHandle) {}
    
    /// 服务器停止时的回调
    fn on_server_stopped(&self, _app: &AppHandle) {}
    
    /// 更新服务器状态（用于托盘图标等）
    fn update_server_state(&self, _app: &AppHandle, _connected_clients: u32) {}
}

/// 平台能力标识
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlatformCapability {
    /// 支持系统托盘
    SystemTray,
    /// 支持开机自启
    AutoStart,
    /// 需要存储权限
    StoragePermission,
    /// 支持前台服务
    ForegroundService,
}
```

**Step 2: 更新 platform/mod.rs**

```rust
// src-tauri/src/platform/mod.rs
mod types;
mod traits;

pub use types::{StorageInfo, PermissionStatus, ServerStartCheckResult};
pub use traits::{PlatformService, PlatformCapability};

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "android")]
pub mod android;

// 平台实例获取
#[cfg(target_os = "windows")]
pub fn get_platform() -> &'static impl PlatformService {
    &windows::WindowsPlatform
}

#[cfg(target_os = "android")]
pub fn get_platform() -> &'static impl PlatformService {
    &android::AndroidPlatform
}

#[cfg(not(any(target_os = "windows", target_os = "android")))]
pub fn get_platform() -> &'static impl PlatformService {
    &DefaultPlatform
}

#[cfg(not(any(target_os = "windows", target_os = "android")))]
struct DefaultPlatform;
```

**Step 3: 为 Windows 实现 Trait**

```rust
// 在 src-tauri/src/platform/windows.rs 末尾添加

use super::traits::{PlatformService, PlatformCapability};
use super::types::{StorageInfo, PermissionStatus};

/// Windows 平台实现
pub struct WindowsPlatform;

impl PlatformService for WindowsPlatform {
    fn name(&self) -> &'static str {
        "windows"
    }
    
    fn setup(&self, app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
        setup_tray(app)?;
        tracing::info!("Windows platform initialized");
        Ok(())
    }
    
    fn get_storage_info(&self) -> StorageInfo {
        StorageInfo {
            display_name: "本地存储".to_string(),
            path: String::new(),
            exists: true,
            writable: true,
            has_all_files_access: true,
        }
    }
    
    fn check_permission_status(&self) -> PermissionStatus {
        PermissionStatus {
            has_all_files_access: true,
            needs_user_action: false,
        }
    }
    
    fn ensure_storage_ready(&self) -> Result<String, String> {
        Ok(String::new())
    }
    
    fn on_server_started(&self, app: &AppHandle) {
        if let Err(e) = update_tray_icon(app, TrayIconState::Idle) {
            tracing::warn!("Failed to update tray icon: {}", e);
        }
    }
    
    fn on_server_stopped(&self, app: &AppHandle) {
        if let Err(e) = update_tray_icon(app, TrayIconState::Stopped) {
            tracing::warn!("Failed to update tray icon: {}", e);
        }
    }
    
    fn update_server_state(&self, app: &AppHandle, connected_clients: u32) {
        let state = if connected_clients > 0 {
            TrayIconState::Active
        } else {
            TrayIconState::Idle
        };
        if let Err(e) = update_tray_icon(app, state) {
            tracing::warn!("Failed to update tray icon: {}", e);
        }
    }
}
```

**Step 4: 为 Android 实现 Trait**

```rust
// 在 src-tauri/src/platform/android.rs 末尾添加

use super::traits::PlatformService;
use super::types::{StorageInfo, PermissionStatus};

/// Android 平台实现
pub struct AndroidPlatform;

impl PlatformService for AndroidPlatform {
    fn name(&self) -> &'static str {
        "android"
    }
    
    fn setup(&self, _app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
        tracing::info!("Android platform initialized");
        Ok(())
    }
    
    fn get_storage_info(&self) -> StorageInfo {
        get_storage_info()
    }
    
    fn check_permission_status(&self) -> PermissionStatus {
        check_permission_status()
    }
    
    fn ensure_storage_ready(&self) -> Result<String, String> {
        ensure_storage_ready()
    }
    
    fn on_server_started(&self, app: &AppHandle) {
        start_foreground_service(app);
    }
    
    fn on_server_stopped(&self, app: &AppHandle) {
        stop_foreground_service(app);
    }
}
```

**Step 5: 重构 lib.rs 使用 Trait**

```rust
// src-tauri/src/lib.rs 关键修改

use platform::get_platform;

fn run() {
    // ... setup_logging 等代码 ...
    
    tauri::Builder::default()
        // ... plugins ...
        .setup(move |app| {
            let platform = get_platform();
            
            // 统一初始化
            if let Err(e) = platform.setup(app.handle()) {
                eprintln!("Platform setup failed: {}", e);
            }
            
            // ... 其他 setup 代码 ...
            
            // 使用 trait 方法替代内联 #[cfg]
            // 服务器启动后调用
            // platform.on_server_started(app.handle());
            
            Ok(())
        })
        // ...
}
```

**Step 6: 验证编译**

Run: `cd src-tauri && cargo build`
Expected: 编译成功

**Step 7: 提交**

```bash
git add src-tauri/src/platform/traits.rs src-tauri/src/platform/mod.rs src-tauri/src/platform/windows.rs src-tauri/src/platform/android.rs src-tauri/src/lib.rs
git commit -m "refactor: add Platform trait abstraction layer"
```

---

### Task 7: 统一构建脚本

**Files:**
- Create: `build.sh`（统一入口）
- Modify: `build-full.sh`（标记为废弃）
- Modify: `build-windows.sh`（标记为废弃）
- Modify: `build-android.sh`（标记为废弃）

**Step 1: 创建统一构建脚本**

```bash
#!/bin/bash
# build.sh - 统一构建入口
set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

usage() {
    echo "用法: ./build.sh <command> [options]"
    echo ""
    echo "命令:"
    echo "  windows           构建 Windows 可执行文件"
    echo "  windows-bundle    构建 Windows 安装包 (EXE + MSI)"
    echo "  android           构建 Android APK (debug)"
    echo "  android-release   构建 Android APK (release)"
    echo "  android-aab       构建 Android AAB (Google Play)"
    echo "  dev               启动开发模式"
    echo "  frontend          仅构建前端"
    echo ""
    echo "示例:"
    echo "  ./build.sh windows"
    echo "  ./build.sh android-release"
    exit 1
}

# 检查命令
if [ $# -eq 0 ]; then
    usage
fi

COMMAND=$1
shift

case $COMMAND in
    windows)
        echo -e "${GREEN}构建 Windows 可执行文件...${NC}"
        ./build-full.sh
        ;;
    windows-bundle)
        echo -e "${GREEN}构建 Windows 安装包...${NC}"
        ./build-windows-bundle.sh
        ;;
    android)
        echo -e "${GREEN}构建 Android APK (debug)...${NC}"
        ./build-android.sh debug
        ;;
    android-release)
        echo -e "${GREEN}构建 Android APK (release)...${NC}"
        ./build-android.sh release
        ;;
    android-aab)
        echo -e "${GREEN}构建 Android AAB...${NC}"
        ./build-android.sh aab
        ;;
    dev)
        echo -e "${GREEN}启动开发模式...${NC}"
        cargo tauri dev
        ;;
    frontend)
        echo -e "${GREEN}构建前端...${NC}"
        bun install --no-cache
        bun run build
        ;;
    *)
        echo -e "${RED}未知命令: $COMMAND${NC}"
        usage
        ;;
esac
```

**Step 2: 设置执行权限**

```bash
chmod +x build.sh
```

**Step 3: 验证脚本**

Run: `./build.sh`
Expected: 显示帮助信息

**Step 4: 提交**

```bash
git add build.sh
git commit -m "feat: add unified build script entry point"
```

---

### Task 8: 合并前端监听器系统

**Files:**
- Modify: `src/stores/serverStore.ts:83-146`
- Modify: `src/App.tsx`（移除 useTauriListeners 调用）
- Modify: `src/hooks/useTauriListeners.ts`（标记为废弃或删除）

**Step 1: 扩展 serverStore 监听器**

```typescript
// src/stores/serverStore.ts 添加更多监听器

initializeListeners: async () => {
  const listeners: UnlistenFn[] = [];

  // 服务器状态事件
  const unlistenStarted = await listen<[string, number]>('server-started', (event) => {
    const [ip, port] = event.payload;
    set({
      isRunning: true,
      serverInfo: {
        is_running: true,
        ip,
        port,
        url: `ftp://${ip}:${port}`,
        username: 'anonymous',
        password_info: '(任意密码)',
      },
      stats: { ...get().stats, is_running: true }
    });
  });
  listeners.push(unlistenStarted);

  const unlistenStopped = await listen('server-stopped', () => {
    set({ isRunning: false, serverInfo: null, stats: defaultStats });
  });
  listeners.push(unlistenStopped);

  // 统计更新
  const unlistenStats = await listen<ServerStatus>('stats-update', (event) => {
    set({ stats: event.payload });
  });
  listeners.push(unlistenStats);

  // 文件上传（Android 媒体扫描）
  const unlistenFileUploaded = await listen<{ path: string; size: number }>('file-uploaded', (event) => {
    // @ts-ignore - Android JS Bridge
    if (window.FileUploadAndroid?.onFileUploaded) {
      try {
        // @ts-ignore
        window.FileUploadAndroid.onFileUploaded(event.payload.path, event.payload.size);
      } catch (err) {
        console.error('Failed to trigger media scan:', err);
      }
    }
  });
  listeners.push(unlistenFileUploaded);

  // 托盘菜单事件
  const unlistenTrayStart = await listen('tray-start-server', async () => {
    try {
      await get().startServer();
    } catch (err) {
      console.error('Failed to start server from tray:', err);
    }
  });
  listeners.push(unlistenTrayStart);

  const unlistenTrayStop = await listen('tray-stop-server', async () => {
    try {
      await get().stopServer();
    } catch (err) {
      console.error('Failed to stop server from tray:', err);
    }
  });
  listeners.push(unlistenTrayStop);

  // 窗口关闭请求
  const unlistenCloseRequested = await listen('window-close-requested', () => {
    set({ showQuitDialog: true });
  });
  listeners.push(unlistenCloseRequested);

  // Android 设置页面请求
  const unlistenOpenSettings = await listen('android-open-manage-storage-settings', () => {
    // @ts-ignore - Android JS Bridge
    if (window.SAFPickerAndroid?.openAllFilesAccessSettings) {
      try {
        // @ts-ignore
        window.SAFPickerAndroid.openAllFilesAccessSettings();
      } catch (err) {
        console.error('Failed to open settings:', err);
      }
    }
  });
  listeners.push(unlistenOpenSettings);

  return async () => {
    for (const unlisten of listeners) {
      unlisten();
    }
  };
},
```

**Step 2: 更新类型定义**

```typescript
// src/types/index.ts 添加
declare global {
  interface Window {
    FileUploadAndroid?: {
      onFileUploaded: (path: string, size: number) => void;
    };
    SAFPickerAndroid?: {
      openAllFilesAccessSettings: () => void;
    };
  }
}
```

**Step 3: 简化 App.tsx**

移除 useTauriListeners 调用，因为所有监听器现在都在 serverStore 中：

```typescript
// src/App.tsx
// 删除 useTauriListeners 导入
// 删除 useTauriListeners([...]) 调用

// 仅保留 serverStore 初始化
useEffect(() => {
  const cleanup = serverStore.getState().initializeListeners();
  return () => {
    cleanup.then(fn => fn());
  };
}, []);
```

**Step 4: 验证构建**

Run: `bun run build`
Expected: 构建成功

**Step 5: 提交**

```bash
git add src/stores/serverStore.ts src/types/index.ts src/App.tsx
git commit -m "refactor: consolidate all event listeners into serverStore"
```

---

## 执行顺序总结

```
Phase 1 (P0 - 立即执行):
├── Task 1: 删除 StorageSettings.tsx
└── Task 2: 修复 minSdkVersion 配置冲突

Phase 2 (P1 - 高优先级):
├── Task 3: 提取 formatBytes 工具函数
├── Task 4: 合并双重错误类型层级
└── Task 5: 统一类型定义

Phase 3 (P2 - 架构优化):
├── Task 6: 实现 Platform Trait 抽象层
├── Task 7: 统一构建脚本
└── Task 8: 合并前端监听器系统
```

---

## 风险与注意事项

1. **Task 4 (错误类型合并)**: 需要仔细检查所有 FtpError 使用点，确保不遗漏
2. **Task 6 (Platform Trait)**: 需要同时测试 Windows 和 Android 构建
3. **Task 8 (监听器合并)**: 需要验证所有事件都能正确触发

---

## 验证清单

每个 Phase 完成后执行：

- [ ] `bun run build` - 前端构建成功
- [ ] `cd src-tauri && cargo build` - Rust 编译成功
- [ ] `cd src-tauri && cargo test` - 单元测试通过
- [ ] Windows 可执行文件正常运行
- [ ] Android APK 正常运行（如适用）
