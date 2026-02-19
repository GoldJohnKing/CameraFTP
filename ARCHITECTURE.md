# Camera FTP Companion - 项目文档

## 项目概述

Camera FTP Companion（图传伴侣）是一款跨平台的相机FTP伴侣应用，支持Windows和Android双平台。它内置FTP服务器，允许相机通过WiFi直接上传照片到电脑或手机，实现即拍即传的摄影工作流。

---

## 架构设计

### 整体架构

```
┌─────────────────────────────────────────────────────────────┐
│                      UI Layer (Web)                          │
│              React + TypeScript + TailwindCSS               │
│                                                              │
│   ┌──────────────┐  ┌──────────────┐  ┌──────────────┐     │
│   │ ServerCard   │  │  StatsCard   │  │  InfoCard    │     │
│   │   (开关)      │  │ (传输统计)    │  │ (连接信息)    │     │
│   └──────────────┘  └──────────────┘  └──────────────┘     │
└────────────────────────────┬────────────────────────────────┘
                             │ Tauri IPC (Command / Event)
┌────────────────────────────┴────────────────────────────────┐
│                   Tauri Runtime                              │
│  ┌─────────────────┐          ┌──────────────────────────┐  │
│  │   Windows       │          │       Android            │  │
│  │  (WebView2)     │          │   (WKWebView)            │  │
│  └─────────────────┘          └──────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                             │
┌────────────────────────────┴────────────────────────────────┐
│              Rust Core Library                               │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐   │
│  │ FtpServer    │  │   Stats      │  │    Network       │   │
│  │   Module     │  │  Collector   │  │   Manager        │   │
│  │              │  │              │  │                  │   │
│  │ - Passive    │  │ - Conn count │  │ - IP detection   │   │
│  │   mode only  │  │ - File count │  │ - Port check     │   │
│  │ - Single     │  │ - Byte count │  │ - Interface      │   │
│  │   session    │  │ - Last file  │  │   listing        │   │
│  └──────────────┘  └──────────────┘  └──────────────────┘   │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐   │
│  │   Storage    │  │   Config     │  │   File Handler   │   │
│  │  (保存路径)   │  │   Manager    │  │   (自动打开)     │   │
│  └──────────────┘  └──────────────┘  └──────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

### 技术选型理由

| 层级 | 技术 | 选型理由 |
|------|------|----------|
| **框架** | Tauri v2 | 跨平台支持、轻量级、Rust核心、支持移动端 |
| **前端** | React + TS | 组件化开发、类型安全、生态丰富 |
| **后端** | Rust | 高性能、内存安全、FTP服务器实现 |
| **样式** | TailwindCSS | 原子化CSS、快速开发、响应式设计 |
| **构建** | Vite | 快速热更新、现代ESM支持 |

---

## 工具链

### 开发环境要求

| 工具 | 版本 | 用途 | 安装命令 |
|------|------|------|----------|
| **Rust** | ≥1.75 | 后端开发 | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| **Node.js** | ≥18 | 前端运行时 | 官网下载或使用nvm |
| **Bun** | ≥1.0 | 包管理器 | `curl -fsSL https://bun.sh/install \| bash` |
| **Tauri CLI** | v2.x | 构建工具 | `cargo install tauri-cli` |
| **Git** | 任意 | 版本控制 | 系统包管理器 |

### Windows特定要求

- **Visual Studio Build Tools** 或 **Visual Studio 2022**
- **Windows SDK**
- **WebView2 Runtime** (Windows 11已预装)

### 可选工具

- **VS Code** + 插件:
  - Rust Analyzer
  - TypeScript Importer
  - Tailwind CSS IntelliSense
  - Tauri

---

## 功能特性

### 核心功能

| 功能 | 描述 | 状态 |
|------|------|------|
| **FTP服务器** | 被动模式FTP服务器，支持相机上传 | ✅ |
| **自动配置** | 自动检测IP地址，智能选择可用端口 | ✅ |
| **传输统计** | 实时显示连接数、文件数、数据量 | ✅ |
| **连接信息** | 一键复制相机配置信息 | ✅ |
| **后台运行** | Windows系统托盘，Android前台服务 | ✅ |
| **配置持久化** | 自动保存用户配置 | ✅ |

### 支持的相机

- 任何支持FTP传输的相机
- 包括Canon、Nikon、Sony、Fujifilm等主流品牌
- 支持WiFi连接和FTP客户端模式

### FTP协议支持

| 命令 | 支持 | 说明 |
|------|------|------|
| `USER/PASS` | ✅ | 匿名登录 |
| `PASV` | ✅ | 被动模式 |
| `STOR` | ✅ | 上传文件 |
| `PWD/CWD` | ✅ | 目录切换 |
| `TYPE/MODE/STRU` | ✅ | 传输设置 |
| `QUIT` | ✅ | 断开连接 |
| `RETR` | ❌ | 下载文件（不需要） |
| `DELE` | ❌ | 删除文件（不需要） |

---

## 项目结构

```
camera-ftp-companion/
├── 📄 配置文件
│   ├── package.json           # Node.js依赖
│   ├── tsconfig.json          # TypeScript配置
│   ├── vite.config.ts         # Vite构建配置
│   ├── tailwind.config.js     # TailwindCSS配置
│   ├── index.html             # HTML入口
│   └── build-full.sh          # 完整构建脚本
│
├── 📁 src/                    # 前端源码
│   ├── main.tsx              # React入口
│   ├── App.tsx               # 主应用组件
│   ├── index.css             # 全局样式
│   ├── types/
│   │   └── index.ts          # TypeScript类型定义
│   └── components/
│       ├── ServerCard.tsx    # 服务器控制卡片
│       ├── StatsCard.tsx     # 统计显示卡片
│       └── InfoCard.tsx      # 连接信息卡片
│
├── 📁 src-tauri/             # Rust后端源码
│   ├── Cargo.toml            # Rust依赖
│   ├── tauri.conf.json       # Tauri配置
│   ├── build.rs              # 构建脚本
│   └── src/
│       ├── main.rs           # 程序入口
│       ├── lib.rs            # 库入口
│       ├── commands.rs       # Tauri命令
│       ├── config.rs         # 配置管理
│       ├── network.rs        # 网络管理
│       ├── ftp/              # FTP模块
│       │   ├── mod.rs        # 模块入口
│       │   ├── server.rs     # FTP服务器
│       │   ├── session.rs    # FTP会话处理
│       │   └── protocol.rs   # FTP协议定义
│       └── platform/         # 平台适配
│           ├── windows.rs    # Windows特定功能
│           └── android.rs    # Android特定功能
│
├── 📁 src-tauri/icons/       # 应用图标
│   ├── icon.ico              # Windows图标
│   ├── icon.icns             # macOS图标
│   ├── 32x32.png
│   └── 128x128.png
│
├── 📁 dist/                  # 前端构建输出
├── 📁 release/               # 发布包
│   ├── camera-ftp-companion.exe
│   └── README.md
│
└── 📁 docs/                  # 文档
    └── plans/                # 设计和计划文档
```

---

## 编译方式

### 开发环境编译

#### 1. 克隆仓库

```bash
git clone <repository-url>
cd camera-ftp-companion
```

#### 2. 安装依赖

```bash
# 安装前端依赖
bun install --no-cache

# 确保Rust已安装
rustc --version
cargo --version
```

#### 3. 开发模式

```bash
# 同时运行前端和后端（热重载）
cargo tauri dev
```

### 生产构建

#### 完整构建（推荐）

```bash
# 一键构建完整项目
./build-full.sh
```

该脚本会：
1. 安装前端依赖
2. 构建前端（输出到 `dist/`）
3. 编译Rust后端（嵌入前端资源）
4. 生成Windows EXE

#### 手动构建

```bash
# 1. 构建前端
bun run build

# 2. 构建Windows EXE
cd src-tauri
cargo build --release --target x86_64-pc-windows-msvc
```

#### WSL交叉编译

在WSL中使用Windows Rust工具链：

```bash
# 使用Windows的cargo
export WINDOWS_CARGO="/mnt/c/Users/$USER/.cargo/bin/cargo.exe"
$WINDOWS_CARGO build --release --target x86_64-pc-windows-msvc
```

### 构建输出

| 平台 | 输出路径 | 文件 |
|------|----------|------|
| Windows | `src-tauri/target/x86_64-pc-windows-msvc/release/` | `camera-ftp-companion.exe` |
| 调试版 | `src-tauri/target/debug/` | `camera-ftp-companion.exe` |

---

## 开发指南

### 添加新功能

#### 添加新的Tauri命令

1. 在 `src-tauri/src/commands.rs` 中添加命令：

```rust
#[command]
pub async fn my_command(arg: String) -> Result<String, String> {
    // 实现逻辑
    Ok(result)
}
```

2. 在 `src-tauri/src/lib.rs` 中注册：

```rust
.invoke_handler(tauri::generate_handler![
    // ... existing commands
    my_command,
])
```

3. 在前端调用：

```typescript
import { invoke } from '@tauri-apps/api/core';

const result = await invoke<string>('my_command', { arg: 'value' });
```

#### 添加新的React组件

1. 在 `src/components/` 创建组件文件
2. 在 `src/App.tsx` 中导入和使用
3. 使用TailwindCSS进行样式设计

### 调试

#### Rust后端调试

```bash
# 添加日志输出
cd src-tauri
RUST_LOG=debug cargo run
```

#### 前端调试

```bash
# 开发服务器（自动热重载）
bun run dev
```

### 配置说明

配置文件位置：
- **Windows**: `%APPDATA%\camera-ftp-companion\config.json`
- **Linux**: `~/.config/camera-ftp-companion/config.json`
- **macOS**: `~/Library/Application Support/camera-ftp-companion/config.json`

配置项：

```json
{
  "save_path": "/path/to/photos",
  "auto_open": true,
  "auto_open_program": null,
  "port": 21,
  "file_extensions": ["jpg", "jpeg", "raw", "png"]
}
```

---

## 发布流程

### 1. 版本号更新

更新以下文件的版本号：
- `package.json`
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`

### 2. 构建发布包

```bash
./build-full.sh
```

### 3. 测试

- [ ] 启动服务器
- [ ] 相机连接测试
- [ ] 文件上传测试
- [ ] 统计显示验证
- [ ] 配置持久化验证
- [ ] 后台运行测试

### 4. 打包分发

```bash
# 创建发布包
mkdir -p release
cp src-tauri/target/x86_64-pc-windows-msvc/release/camera-ftp-companion.exe release/
cp README.md release/
```

---

## 常见问题

### Q: 编译时提示缺少WebView2？
**A:** Windows 11已预装WebView2。对于Windows 10，需要从微软官网下载安装。

### Q: 端口21被占用？
**A:** 应用会自动检测并切换到下一个可用端口（1025+），无需手动配置。

### Q: 无法获取IP地址？
**A:** 确保电脑已连接到WiFi或以太网。应用会自动选择最佳网络接口。

### Q: 相机无法连接？
**A:** 检查：
1. 电脑和相机在同一网络
2. 防火墙允许应用访问网络
3. 使用正确的IP和端口
4. 选择被动模式（PASV）

### Q: 如何更改保存路径？
**A:** 在设置中修改，或手动编辑配置文件。更改后重启服务器生效。

---

## 技术参考

### 相关文档

- [Tauri v2 文档](https://tauri.app/)
- [Rust 文档](https://doc.rust-lang.org/)
- [React 文档](https://react.dev/)
- [TailwindCSS 文档](https://tailwindcss.com/)

### FTP协议参考

- [RFC 959 - FTP协议](https://tools.ietf.org/html/rfc959)
- [被动模式详解](https://slacksite.com/other/ftp.html)

---

## 贡献指南

### 提交规范

使用 [Conventional Commits](https://www.conventionalcommits.org/)：

```
feat: 添加新功能
fix: 修复bug
docs: 更新文档
style: 代码格式调整
refactor: 重构代码
test: 添加测试
chore: 构建/工具更新
```

### 代码风格

- **Rust**: 使用 `rustfmt` 和 `clippy`
- **TypeScript**: 使用项目配置的ESLint规则
- **提交前**: 运行 `cargo check` 和 `bun run build`

---

## 许可证

[添加许可证信息]

---

**文档版本**: 1.0  
**更新日期**: 2025-02-19  
**作者**: Camera FTP Companion Team