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
│   ├── components/
│   │   ├── ServerCard.tsx        # 服务器控制卡片（启动/停止）
│   │   ├── StatsCard.tsx         # 统计显示卡片
│   │   ├── InfoCard.tsx          # 连接信息卡片
│   │   ├── ConfigCard.tsx        # 配置管理卡片
│   │   ├── BottomNav.tsx         # 底部导航栏
│   │   └── StorageSettings.tsx   # 存储设置组件
│   ├── stores/
│   │   ├── serverStore.ts        # 服务器状态管理（Zustand）
│   │   └── configStore.ts        # 配置状态管理
│   └── hooks/
│       ├── useTauriListeners.ts  # Tauri事件监听Hook
│       └── useStoragePermission.ts # 存储权限管理Hook
│
├── 📁 src-tauri/                 # Rust后端源码
│   ├── Cargo.toml                # Rust依赖配置
│   ├── tauri.conf.json           # Tauri配置
│   ├── build.rs                  # Rust构建脚本
│   ├── mobile.toml               # 移动端Tauri配置
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
│       ├── error.rs              # 错误类型定义
│       ├── storage_permission.rs # 存储权限管理
│       ├── saf_picker.rs         # Android SAF文件选择器
│       ├── ftp/                  # FTP模块
│       │   ├── mod.rs            # 模块入口与测试
│       │   ├── server.rs         # FTP服务器Actor实现
│       │   ├── server_factory.rs # 服务器工厂（启动/停止/事件）
│       │   ├── events.rs         # 事件总线（EventBus）
│       │   ├── listeners.rs      # FTP事件监听器
│       │   ├── stats.rs          # 统计信息Actor
│       │   ├── types.rs          # 类型定义（Config/Stats/Event）
│       │   └── error.rs          # FTP错误类型
│       └── platform/             # 平台适配
│           ├── mod.rs            # 平台模块入口
│           ├── windows.rs        # Windows平台（托盘/自启）
│           └── android.rs        # Android平台（权限/存储）
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
- `serverStore`: 服务器状态（isRunning, stats, start/stop actions）
- `configStore`: 配置状态（savePath, autoStart, activeTab）

**事件监听**:
- `stats-update`: 统计信息更新
- `server-started`/`server-stopped`: 服务器状态变化
- `tray-start-server`/`tray-stop-server`: 托盘菜单操作
- `window-close-requested`: 窗口关闭请求

---

## 构建命令

### ⚠️ 重要：必须使用编译脚本构建

**Agent指令**: 所有平台产物的构建**必须**使用项目提供的编译脚本，不要直接使用cargo或bun命令。

| 平台 | 编译脚本 | 输出 |
|------|----------|------|
| **Windows** | `./build-full.sh` | `src-tauri/target/x86_64-pc-windows-msvc/release/camera-ftp-companion.exe` |
| **Windows Bundle** | `./build-windows-bundle.sh` | EXE + MSI安装包 |
| **Android Debug** | `./build-android.sh debug` | `src-tauri/gen/android/app/build/outputs/apk/` |
| **Android Release** | `./build-android.sh release` | 签名APK |
| **Android AAB** | `./build-android.sh aab` | Google Play AAB包 |

### 开发命令

```bash
# 仅启动前端开发服务器（端口1420）
bun run dev

# 同时运行前端和后端（热重载）
cargo tauri dev

# Android开发模式（热重载）
./build-android.sh dev
```

---

## 代码验证规范

### ⚠️ 重要：依赖编译验证代码正确性

**Agent指令**: **不要使用 `lsp_diagnostics`** 来验证代码正确性。始终依赖**实际编译**来验证代码。

**原因**: LSP诊断有时会滞后或不准确，特别是在跨平台条件编译（`#[cfg(target_os = "windows")]`）和复杂宏展开的场景下。

**正确做法**:
```bash
# 修改Rust代码后，立即编译验证
cd src-tauri && cargo build

# 验证Windows平台
cargo build --target x86_64-pc-windows-msvc

# 验证Android平台  
cargo build --target aarch64-linux-android
```

**前端验证**:
```bash
# TypeScript类型检查
bun run build

# 或
npx tsc --noEmit
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
4. **编译验证**: `cargo build`

### 添加新的React组件

1. 在 `src/components/` 创建组件文件
2. 在 `src/App.tsx` 中导入和使用
3. 使用 TailwindCSS 进行样式设计
4. **编译验证**: `bun run build`

### 修改FTP服务器配置

编辑 `src-tauri/src/ftp/types.rs`:
```rust
pub struct ServerConfig {
    pub port: u16,
    pub root_path: PathBuf,
    pub allow_anonymous: bool,
    pub passive_port_range: (u16, u16),
    pub idle_timeout_seconds: u64,
}
```

### 修改窗口配置

编辑 `src-tauri/tauri.conf.json`:
```json
{
  "app": {
    "windows": [{
      "title": "图传伴侣",
      "width": 400,
      "height": 700
    }]
  }
}
```

---

## 配置文件位置

- **Windows**: `%APPDATA%\camera-ftp-companion\config.json`
- **Android**: `/data/data/com.gjk.cameraftpcompanion/files/config.json`（应用私有目录）

## 照片存储路径

- **Windows**: 用户图片目录下的 `CameraFTP` 文件夹（可配置）
- **Android**: `/storage/emulated/0/DCIM/CameraFTP`（固定路径，需要所有文件访问权限）

---

## 调试技巧

### Rust后端调试
```bash
cd src-tauri
RUST_LOG=debug cargo run
```

### Windows控制台窗口
在 `src-tauri/src/main.rs` 中添加以隐藏控制台:
```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
```

### 前端资源路径
确保 `vite.config.ts` 中包含:
```typescript
base: "./",
```

### Android调试
```bash
# 查看日志
adb logcat -s "RustStdoutStderr" "camera-ftp-companion"

# 检查权限状态
adb shell appops get com.gjk.cameraftpcompanion
```

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
2. **代码验证**: 不要使用 `lsp_diagnostics`，始终依赖 `cargo build` 或 `bun run build` 进行验证
3. **平台代码**: 使用 `#[cfg(target_os = "...")]` 进行条件编译
4. **日志记录**: 使用 `tracing::info!`, `tracing::error!` 等宏
5. **错误处理**: 使用 `Result<T, AppError>` 和 `?` 操作符
