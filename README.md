# 📸 Camera FTP Companion（图传伴侣）

一款跨平台的相机FTP伴侣应用，让相机照片直接传输到电脑或手机。

![版本](https://img.shields.io/badge/version-0.1.0-blue)
![平台](https://img.shields.io/badge/platform-Windows%20%7C%20Android-brightgreen)
![技术栈](https://img.shields.io/badge/tech-Tauri%20%2B%20React%20%2B%20Rust-orange)

---

## ✨ 功能特性

- 🚀 **一键启动** - 自动检测IP，智能选择可用端口
- 📡 **FTP服务器** - 内置高性能FTP服务，支持相机WiFi直传
- 📊 **实时统计** - 显示连接状态、已接收照片数、数据量
- 📋 **连接信息** - 一键复制相机配置（IP/端口/用户名）
- 🖥️ **系统托盘** - Windows后台运行，开机自启支持
- 🔔 **状态指示** - 托盘图标颜色显示服务器状态（红/黄/绿）
- 🔐 **权限管理** - Android 11+所有文件访问权限
- 💾 **配置持久化** - 自动保存设置，跨会话保持

---

## 🏗️ 技术架构

```
React + TypeScript + TailwindCSS (前端)
           ↓
     Tauri IPC (Command/Event)
           ↓
Rust + libunftp (FTP Server后端)
           ↓
Kotlin + JS Bridge (Android原生)
```

| 层级 | 技术 | 版本 |
|------|------|------|
| **框架** | Tauri v2 | ^2.0.0 |
| **前端** | React | ^18.2.0 |
| **语言** | TypeScript | ^5.0.2 |
| **状态管理** | Zustand | ^5.0.11 |
| **样式** | TailwindCSS | ^3.4.15 |
| **构建工具** | Vite | ^5.0.0 |
| **后端** | Rust | ≥1.75 |
| **FTP服务器** | libunftp | 0.23.0 |
| **Android Native** | Kotlin | 1.9.25 |
| **Android API Level** | min 30 / target 36 | Android 11+ |
| **Java** | JDK | 17 |

---

## 📁 项目结构

```
camera-ftp-companion/
├── 📄 配置文件
│   ├── package.json              # Node.js依赖
│   ├── build.sh                  # ⭐ 统一构建入口
│   ├── build-windows.sh          # Windows构建
│   ├── build-android.sh          # Android构建
│   └── ...
│
├── 📁 src/                       # React前端源码
│   ├── main.tsx                  # React入口
│   ├── App.tsx                   # 主应用组件
│   ├── components/               # UI组件
│   ├── stores/                   # Zustand状态管理
│   └── utils/                    # 工具函数
│
├── 📁 src-tauri/                 # Rust后端源码
│   ├── Cargo.toml                # Rust依赖
│   ├── src/
│   │   ├── main.rs               # 程序入口
│   │   ├── commands.rs           # Tauri命令（IPC接口）
│   │   ├── ftp/                  # FTP服务器实现
│   │   └── platform/             # 平台适配（Windows/Android）
│   │
│   └── 📁 gen/android/           # Android原生代码 (Kotlin)
│       └── app/src/main/java/com/gjk/cameraftpcompanion/
│           ├── MainActivity.kt           # 主活动 + JS Bridge
│           ├── FtpForegroundService.kt   # FTP前台服务
│           ├── PermissionBridge.kt       # 权限JS Bridge
│           ├── StorageHelper.kt          # 存储辅助
│           └── MediaScannerHelper.kt     # 媒体扫描
│
└── 📁 dist/                      # 构建输出
```

---

## 🤖 Android 原生代码

Android平台使用Kotlin实现以下功能：

| 文件 | 功能 |
|------|------|
| **MainActivity.kt** | 主活动，管理JS Bridge（文件上传、服务器状态、存储设置） |
| **FtpForegroundService.kt** | 前台服务，保持FTP在后台运行，显示状态通知 |
| **PermissionBridge.kt** | 权限管理（存储、通知、电池优化） |
| **StorageHelper.kt** | 跳转到系统存储权限设置页面 |
| **MediaScannerHelper.kt** | 文件上传后触发媒体扫描，让照片出现在相册 |

### JS Bridge 说明

前端通过以下Bridge与Android原生交互：

```typescript
// 存储权限设置
window.StorageSettingsAndroid?.openAllFilesAccessSettings()

// 权限检查
window.PermissionAndroid?.checkAllPermissions()
window.PermissionAndroid?.requestStoragePermission()

// 文件上传事件（由Rust通过Tauri事件触发）
window.FileUploadAndroid?.onFileUploaded(path, size)

// 服务器状态更新
window.ServerStateAndroid?.onServerStateChanged(isRunning, statsJson, connectedClients)
```

---

## 🚀 快速开始

### 环境要求

- **Rust** ≥1.75
- **Node.js** ≥18
- **Bun** ≥1.0

### 开发模式

```bash
# 安装依赖
bun install

# 启动开发服务器
cargo tauri dev
```

### 生产构建

```bash
# 统一构建入口（必须使用）
./build.sh <command>

# 常用命令：
./build.sh windows         # Windows 可执行文件
./build.sh windows-bundle  # Windows 安装包 (EXE + MSI)
./build.sh android         # Android APK (release)
./build.sh android-debug   # Android APK (debug)
./build.sh android-aab     # Android AAB (Google Play)
./build.sh frontend        # 仅构建前端
./build.sh dev             # 启动开发模式
```

---

## ⚙️ 配置与存储

### 配置文件位置

- **Windows**: `%APPDATA%\camera-ftp-companion\config.json`
- **Android**: `/data/data/com.gjk.cameraftpcompanion/files/config.json`

### 照片存储路径

- **Windows**: 用户图片目录下的 `CameraFTP` 文件夹（可配置）
- **Android**: `/storage/emulated/0/DCIM/CameraFTP`（固定路径）

---

## 🐛 常见问题

**Q: 端口被占用？**  
A: 应用会自动切换到下一个可用端口。

**Q: 相机连接失败？**  
A: 检查防火墙设置，确保电脑和相机在同一网络。

**Q: Android无法保存照片？**  
A: 确保已授予"所有文件访问权限"。

---

## 📄 许可证

MIT © 2025 Camera FTP Companion

---

**让摄影工作流更简单！** 📷
