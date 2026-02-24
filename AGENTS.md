# Camera FTP Companion - OpenCode 基础指令

## 项目概述

**Camera FTP Companion（图传伴侣）** 是一款跨平台的相机FTP伴侣桌面应用，支持Windows和Android双平台。它允许相机通过WiFi直接上传照片到电脑或手机，实现即拍即传的摄影工作流。

### 核心功能
- 🚀 一键启动FTP服务器（自动检测IP，智能选择端口）
- 📡 支持相机直接上传照片
- 📊 实时显示传输统计（连接数、文件数、数据量）
- 📋 一键复制相机配置信息
- 🖥️ Windows系统托盘后台运行（支持开机自启）
- 🔔 托盘图标状态显示（红/黄/绿点表示服务器状态）
- 💾 配置持久化自动保存
- 🔐 Android所有文件访问权限管理

---

## 技术架构

```
React + TypeScript + TailwindCSS (前端)
           ↓
     Tauri IPC (Command/Event)
           ↓
Rust + libunftp (FTP Server后端)
```

| 层级 | 技术 | 版本 | 用途 |
|------|------|------|------|
| **框架** | Tauri v2 | ^2.0.0 | 跨平台桌面应用框架 |
| **前端** | React | ^18.2.0 | UI框架 |
| **前端语言** | TypeScript | ^5.0.2 | 类型安全 |
| **状态管理** | Zustand | ^5.0.11 | 轻量级状态管理 |
| **通知** | Sonner | ^2.0.7 | Toast通知 |
| **样式** | TailwindCSS | ^3.4.15 | 原子化CSS |
| **构建工具** | Vite | ^5.0.0 | 前端构建 |
| **后端** | Rust | ≥1.75 | 系统编程 |
| **异步运行时** | Tokio | ^1 | Rust异步处理 |
| **FTP服务器** | libunftp | 0.23.0 | FTP协议实现 |
| **包管理** | Bun | ≥1.0 | Node.js包管理 |

---

## 目录结构

```
camera-ftp-companion/
├── 📄 配置文件
│   ├── package.json              # Node.js依赖和脚本
│   ├── tsconfig.json             # TypeScript配置
│   ├── vite.config.ts            # Vite构建配置
│   ├── tailwind.config.js        # TailwindCSS配置
│   ├── index.html                # HTML入口
│   ├── build.sh                  # ⭐ 统一构建入口（推荐使用）
│   ├── build-full.sh             # 完整构建脚本（Windows+前端）
│   ├── build-windows.sh          # Windows构建脚本
│   ├── build-windows-bundle.sh   # Windows安装包构建
│   └── build-android.sh          # Android构建脚本
│
├── 📁 src/                       # React前端源码
│   ├── main.tsx                  # React入口
│   ├── App.tsx                   # 主应用组件（含退出对话框逻辑）
│   ├── index.css                 # 全局样式
│   ├── types/
│   │   └── index.ts              # TypeScript类型定义
│   ├── utils/
│   │   └── format.ts             # 共享工具函数（formatBytes等）
│   ├── components/
│   │   ├── ServerCard.tsx        # 服务器控制卡片（启动/停止）
│   │   ├── StatsCard.tsx         # 统计显示卡片
│   │   ├── InfoCard.tsx          # 连接信息卡片
│   │   ├── ConfigCard.tsx        # 配置管理卡片（含存储设置）
│   │   └── BottomNav.tsx         # 底部导航栏
│   ├── stores/
│   │   ├── serverStore.ts        # 服务器状态管理 + 统一事件监听
│   │   └── configStore.ts        # 配置状态管理
│   └── hooks/
│       ├── useTauriListeners.ts  # Tauri事件监听Hook（可选使用）
│       └── useStoragePermission.ts # 存储权限管理Hook
│
├── 📁 src-tauri/                 # Rust后端源码
│   ├── Cargo.toml                # Rust依赖配置
│   ├── tauri.conf.json           # Tauri配置
│   ├── build.rs                  # Rust构建脚本
│   ├── mobile.toml               # 移动端Tauri配置（minSdk=30）
│   ├── icons/                    # 应用图标
│   │   ├── tray-stopped.png      # 托盘图标-停止（红点）
│   │   ├── tray-idle.png         # 托盘图标-空闲（黄点）
│   │   └── tray-active.png       # 托盘图标-活跃（绿点）
│   ├── capabilities/             # Tauri能力配置
│   │   └── default.json          # 默认能力声明
│   └── src/
│       ├── main.rs               # 程序入口
│       ├── lib.rs                # 库入口（Tauri Builder配置）
│       ├── commands.rs           # Tauri命令定义（IPC接口）
│       ├── config.rs             # 应用配置管理
│       ├── network.rs            # 网络管理（IP/端口检测）
│       ├── error.rs              # 统一错误类型（AppError）
│       ├── storage_permission.rs # 存储权限管理命令
│       ├── saf_picker.rs         # Android SAF文件选择器
│       ├── ftp/                  # FTP模块
│       │   ├── mod.rs            # 模块入口与测试
│       │   ├── server.rs         # FTP服务器Actor实现
│       │   ├── server_factory.rs # 服务器工厂（启动/停止/事件）
│       │   ├── events.rs         # 事件总线（EventBus）
│       │   ├── listeners.rs      # FTP事件监听器
│       │   ├── stats.rs          # 统计信息Actor
│       │   ├── types.rs          # 类型定义（Config/Stats/Event）
│       │   └── error.rs          # FTP专用错误（仅BindFailed/Io）
│       └── platform/             # 平台适配（Trait抽象层）
│           ├── mod.rs            # 平台模块入口 + get_platform()
│           ├── types.rs          # 统一类型（StorageInfo等）
│           ├── traits.rs         # PlatformService Trait定义
│           ├── windows.rs        # Windows平台实现
│           └── android.rs        # Android平台实现
│
├── 📁 dist/                      # 前端构建输出
└── 📁 release/                   # 发布包
```

---

## 模块详细说明

### FTP模块 (`src-tauri/src/ftp/`)

采用**Actor模式**实现，保证线程安全和状态隔离：

- **FtpServerActor**: 服务器主Actor，处理Start/Stop/GetStatus等命令
- **StatsActor**: 统计信息Actor，独立处理上传统计
- **EventBus**: 事件总线，广播ServerStarted/FileUploaded等事件
- **FtpDataListener/FtpPresenceListener**: FTP事件监听器

**状态流转**: Stopped → Starting → Running → Stopping → Stopped

### 平台适配 (`src-tauri/src/platform/`)

采用 **Trait 抽象层** 统一平台接口，减少内联 `#[cfg]` 散落：

**核心 Trait** (`traits.rs`):
```rust
pub trait PlatformService: Send + Sync {
    fn name(&self) -> &'static str;
    fn setup(&self, app: &AppHandle) -> Result<(), Box<dyn std::error::Error>>;
    fn get_storage_info(&self) -> StorageInfo;
    fn check_permission_status(&self) -> PermissionStatus;
    fn ensure_storage_ready(&self) -> Result<String, String>;
    fn on_server_started(&self, _app: &AppHandle) {}
    fn on_server_stopped(&self, _app: &AppHandle) {}
    fn update_server_state(&self, _app: &AppHandle, _connected_clients: u32) {}
}
```

**统一类型** (`types.rs`):
- `StorageInfo`: 存储路径信息
- `PermissionStatus`: 权限状态
- `ServerStartCheckResult`: 服务器启动检查结果

**平台实现**:
- `WindowsPlatform`: 托盘图标、开机自启、窗口控制
- `AndroidPlatform`: 存储权限、前台服务、固定存储路径

**获取平台实例**:
```rust
use crate::platform::get_platform;
let platform = get_platform();
platform.setup(app.handle())?;
```

**Windows平台** (`windows.rs`):
- 系统托盘实现（托盘图标动态切换）
- 开机自启（注册表操作）
- 开机启动模式检测（--autostart参数）
- 窗口隐藏/显示控制

**Android平台** (`android.rs`):
- 所有文件访问权限检查
- 固定存储路径管理（/storage/emulated/0/DCIM/CameraFTP）
- 设备信息获取

### 前端架构

**状态管理** (Zustand):
- `serverStore`: 服务器状态（isRunning, stats, start/stop actions）+ 统一事件监听
- `configStore`: 配置状态（savePath, autoStart, activeTab）

**工具函数** (`utils/format.ts`):
- `formatBytes()`: 字节数格式化为 MB
- `formatBytesAuto()`: 自动选择最佳单位

**事件监听** (统一在 `serverStore.initializeListeners()`):
- `server-started`/`server-stopped`: 服务器状态变化
- `stats-update`: 统计信息更新
- `file-uploaded`: 文件上传完成（Android 媒体扫描）
- `tray-start-server`/`tray-stop-server`: 托盘菜单操作
- `window-close-requested`: 窗口关闭请求
- `android-open-manage-storage-settings`: Android 设置页面请求

---

## 构建命令

### ⚠️ 重要：必须使用编译脚本构建

**Agent指令**: 所有平台产物的构建**必须**使用项目提供的编译脚本，不要直接使用`cargo`或`bun`命令。

#### 统一构建入口（推荐）

```bash
./build.sh <command>
```

| 命令 | 说明 |
|------|------|
| `./build.sh windows` | 构建 Windows 可执行文件 |
| `./build.sh windows-bundle` | 构建 Windows 安装包 (EXE + MSI) |
| `./build.sh android` | 构建 Android APK (debug) |
| `./build.sh android-release` | 构建 Android APK (release) |
| `./build.sh frontend` | 仅构建前端 |

#### 传统构建脚本

| 平台 | 编译脚本 | 输出 |
|------|----------|------|
| **Windows** | `./build-full.sh` | `src-tauri/target/x86_64-pc-windows-msvc/release/camera-ftp-companion.exe` |
| **Windows Bundle** | `./build-windows-bundle.sh` | EXE + MSI安装包 |
| **Android Debug** | `./build-android.sh debug` | `src-tauri/gen/android/app/build/outputs/apk/` |
| **Android Release** | `./build-android.sh release` | 签名APK |
| **Android AAB** | `./build-android.sh aab` | Google Play AAB包 |

---

## 代码验证规范

### ⚠️ 重要：依赖编译验证代码正确性

**Agent指令**: **不要使用 `lsp_diagnostics`** 来验证代码正确性。始终依赖**实际编译**来验证代码。编译后端时**必须**使用项目提供的编译脚本，不要直接使用`cargo build`命令。

**原因**: LSP诊断存在卡死异常，无法正常运行。编译脚本为后端编译提供了编译环境预处理，直接使用`cargo build`命令可能无法获取必要工具链。

**后端验证**: 修改Rust代码后，立即编译验证
```bash
# 验证Windows平台
./build.sh windows

# 验证Android平台  
./build.sh android-release
```

**前端验证**:
```bash
# TypeScript类型检查
bun run build
```

---

## 代码风格约定

### TypeScript / React

- **目标**: ES2020
- **模块**: ESNext with Bundler resolution
- **JSX**: react-jsx
- **严格模式**: 启用
- **样式**: TailwindCSS 原子类

**React组件示例**:
```typescript
import { useState } from 'react';
import { ServerCard } from './components/ServerCard';

function App() {
  const [serverInfo, setServerInfo] = useState<ServerInfo | null>(null);

  return (
    <div className="min-h-screen bg-gray-50">
      <div className="max-w-md mx-auto p-4">
        <ServerCard onStatusChange={setServerInfo} />
      </div>
    </div>
  );
}
```

### Rust

- **Edition**: 2021
- **格式化**: 使用 `rustfmt`
- **检查**: 使用 `clippy`
- **错误处理**: 使用 `Result<T, E>` 和 `?` 操作符
- **日志**: 使用 `tracing` crate

**Tauri命令示例**:
```rust
#[command]
pub async fn start_server(
    state: State<'_, FtpServerState>,
    app: AppHandle,
) -> Result<ServerInfo, AppError> {
    tracing::info!("Starting FTP server...");
    // 实现逻辑
    Ok(result)
}
```

### 条件编译

**平台特定代码必须使用 `#[cfg(...)]`**:
```rust
#[cfg(target_os = "windows")]
pub fn setup_tray(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    // Windows托盘实现
}

#[cfg(target_os = "android")]
pub fn check_all_files_permission() -> bool {
    // Android权限检查
}
```

### Tauri IPC 调用规范

**前端调用**:
```typescript
import { invoke } from '@tauri-apps/api/core';

const result = await invoke<string>('start_server', { port: 21 });
```

**后端注册** (在 `lib.rs` 中):
```rust
.invoke_handler(tauri::generate_handler![
    start_server,
    stop_server,
    get_server_status,
    // ...
])
```

---

## 常见开发任务

### 添加新的Tauri命令

1. 在 `src-tauri/src/commands.rs` 中添加命令函数
2. 在 `src-tauri/src/lib.rs` 的 `generate_handler!` 中注册
3. 在前端使用 `invoke()` 调用
4. **编译验证**: `./build.sh <command>`

### 添加新的React组件

1. 在 `src/components/` 创建组件文件
2. 在 `src/App.tsx` 中导入和使用
3. 使用 TailwindCSS 进行样式设计
4. **编译验证**: `bun run build`

---

## 配置文件位置

- **Windows**: `%APPDATA%\camera-ftp-companion\config.json`
- **Android**: `/data/data/com.gjk.cameraftpcompanion/files/config.json`（应用私有目录）

## 照片存储路径

- **Windows**: 用户图片目录下的 `CameraFTP` 文件夹（可配置）
- **Android**: `/storage/emulated/0/DCIM/CameraFTP`（固定路径，需要所有文件访问权限）

---

## 提交规范

使用 [Conventional Commits](https://www.conventionalcommits.org/):

```
feat: 添加新功能
fix: 修复bug
docs: 更新文档
style: 代码格式调整
refactor: 重构代码
test: 添加测试
chore: 构建/工具更新
build: 影响构建系统或外部依赖的更改
ci: CI配置更改
```

---

## 参考文档

- [Tauri v2 文档](https://tauri.app/)
- [Tauri Mobile 文档](https://v2.tauri.app/start/migrate/from-tauri-1/)
- [Rust 文档](https://doc.rust-lang.org/)
- [React 文档](https://react.dev/)
- [TailwindCSS 文档](https://tailwindcss.com/)
- [libunftp 文档](https://docs.rs/libunftp/)

---

## Agent 指令总结

1. **构建平台产物**: 必须使用 `./build-full.sh`, `./build-android.sh` 等编译脚本
2. **代码验证**: 不要使用 `lsp_diagnostics`，不要直接使用`cargo build`命令，而是始终使用编译脚本进行验证
3. **平台代码**: 使用 `#[cfg(target_os = "...")]` 进行条件编译
4. **日志记录**: 使用 `tracing::info!`, `tracing::error!` 等宏
5. **错误处理**: 使用 `Result<T, AppError>` 和 `?` 操作符
