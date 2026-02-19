# Camera FTP Companion - OpenCode 基础指令

## 项目概述

**Camera FTP Companion（图传伴侣）** 是一款跨平台的相机FTP伴侣桌面应用，支持Windows和Android双平台。它允许相机通过WiFi直接上传照片到电脑或手机，实现即拍即传的摄影工作流。

### 核心功能
- 🚀 一键启动FTP服务器（自动检测IP，智能选择端口）
- 📡 支持相机直接上传照片
- 📊 实时显示传输统计（连接数、文件数、数据量）
- 📋 一键复制相机配置信息
- 🖥️ Windows系统托盘后台运行
- 💾 配置持久化自动保存

---

## 技术架构

```
React + TypeScript + TailwindCSS (前端)
           ↓
     Tauri IPC (Command/Event)
           ↓
Rust + Tokio (FTP Server后端)
```

| 层级 | 技术 | 版本 | 用途 |
|------|------|------|------|
| **框架** | Tauri v2 | ^2.0.0 | 跨平台桌面应用框架 |
| **前端** | React | ^18.2.0 | UI框架 |
| **前端语言** | TypeScript | ^5.0.2 | 类型安全 |
| **样式** | TailwindCSS | ^3.4.15 | 原子化CSS |
| **构建工具** | Vite | ^5.0.0 | 前端构建 |
| **后端** | Rust | ≥1.75 | 系统编程 |
| **异步运行时** | Tokio | ^1 | Rust异步处理 |
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
│   └── build-full.sh             # 完整构建脚本
│
├── 📁 src/                       # React前端源码
│   ├── main.tsx                  # React入口
│   ├── App.tsx                   # 主应用组件
│   ├── index.css                 # 全局样式
│   ├── types/
│   │   └── index.ts              # TypeScript类型定义
│   └── components/
│       ├── ServerCard.tsx        # 服务器控制卡片
│       ├── StatsCard.tsx         # 统计显示卡片
│       └── InfoCard.tsx          # 连接信息卡片
│
├── 📁 src-tauri/                 # Rust后端源码
│   ├── Cargo.toml                # Rust依赖配置
│   ├── tauri.conf.json           # Tauri配置
│   ├── build.rs                  # Rust构建脚本
│   ├── icons/                    # 应用图标
│   ├── capabilities/             # Tauri能力配置
│   └── src/
│       ├── main.rs               # 程序入口
│       ├── lib.rs                # 库入口
│       ├── commands.rs           # Tauri命令定义
│       ├── config.rs             # 配置管理
│       ├── network.rs            # 网络管理
│       ├── ftp/                  # FTP模块
│       │   ├── mod.rs
│       │   ├── server.rs
│       │   ├── session.rs
│       │   └── protocol.rs
│       └── platform/             # 平台适配
│           ├── windows.rs
│           └── android.rs
│
├── 📁 dist/                      # 前端构建输出
└── 📁 release/                   # 发布包
```

---

## 常用命令

### 开发命令

```bash
# 仅启动前端开发服务器（端口1420）
bun run dev

# 同时运行前端和后端（热重载）
cargo tauri dev
```

### 构建命令

```bash
# 构建前端（输出到 dist/）
bun run build

# 完整构建Windows EXE（使用build-full.sh）
./build-full.sh

# 手动构建Rust后端
cd src-tauri
cargo build --release --target x86_64-pc-windows-msvc
```

### 其他命令

```bash
# 安装依赖
bun install --no-cache

# Tauri CLI帮助
bun run tauri -- --help

# 预览生产构建
bun run preview
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

**Tauri命令示例**:
```rust
#[command]
pub async fn start_server(
    state: State<'_, FtpServerState>,
    port: u16,
) -> Result<String, String> {
    // 实现逻辑
    Ok(result)
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
    get_network_info,
    load_config,
    save_config,
    check_port_available,
])
```

---

## 常见开发任务

### 添加新的Tauri命令

1. 在 `src-tauri/src/commands.rs` 中添加命令函数
2. 在 `src-tauri/src/lib.rs` 的 `generate_handler!` 中注册
3. 在前端使用 `invoke()` 调用

### 添加新的React组件

1. 在 `src/components/` 创建组件文件
2. 在 `src/App.tsx` 中导入和使用
3. 使用 TailwindCSS 进行样式设计

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

### 修改构建配置

- **前端构建**: `vite.config.ts`
- **Rust依赖**: `src-tauri/Cargo.toml`
- **Tauri配置**: `src-tauri/tauri.conf.json`

---

## 配置文件位置

- **Windows**: `%APPDATA%\camera-ftp-companion\config.json`
- **Linux**: `~/.config/camera-ftp-companion/config.json`
- **macOS**: `~/Library/Application Support/camera-ftp-companion/config.json`

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
```

---

## 参考文档

- [Tauri v2 文档](https://tauri.app/)
- [Rust 文档](https://doc.rust-lang.org/)
- [React 文档](https://react.dev/)
- [TailwindCSS 文档](https://tailwindcss.com/)
