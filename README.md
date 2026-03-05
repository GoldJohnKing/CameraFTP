# 📸 CameraFTP（图传伴侣）

一款跨平台的相机FTP伴侣应用，让相机照片直接传输到电脑或手机。

![版本](https://img.shields.io/badge/version-0.1.0-blue)
![平台](https://img.shields.io/badge/platform-Windows%20%7C%20Android-brightgreen)
![技术栈](https://img.shields.io/badge/tech-Tauri%20%2B%20React%20%2B%20Rust-orange)

---

## ✨ 功能特性

- 🚀 **一键启动** - 一键启动，无需复杂配置，自动显示连接信息（IP/端口/用户名）
- 📡 **FTP服务器** - 基于FTP协议，无需蓝牙，相机WiFi直传；支持进阶配置
- 📊 **实时统计** - 实时显示连接状态、最新照片、已接收照片数、数据量

### 🖥️ Windows专属功能

- 🔔 **开机自启** - Windows后台运行，支持开机自启
- 🚦 **状态指示** - 托盘图标颜色显示服务器状态（红/黄/绿）
- 🖼️ **自动预览** - 接收照片后自动打开预览窗口或第三方软件
- 🖼️ **内置预览工具**
  - 📷 **EXIF元数据** - 读取照片拍摄时间，支持 JPG/HEIF/HEIC 等格式
  - 📁 **文件索引** - 启动时自动扫描目录，建立图片索引数据库
  - 👁️ **文件监听** - 实时监听文件系统变化，同步更新索引

### 📱 Android专属功能

- 🔐 **权限导览** - 缺少必要权限时，显示权限配置导览，一键直达授权界面
- 🛡️ **运行保活** - 常驻通知保活，避免进程意外结束

---

## 🏗️ 技术架构

```
    React + TypeScript + TailwindCSS (前端)
                        │
          ┌─────────────┴─────────────┐
          │                           │
          ▼                           ▼
    Tauri IPC                   JS Bridge
    (Command/Event)             (Android)
          │                           │
          ▼                           ▼
    Rust + libunftp             Kotlin
    (FTP Server)                (Android原生服务)
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
| **EXIF读取** | nom-exif | 2.7 |
| **文件监听** | notify | 8.0 |
| **密码哈希** | argon2 | 0.5 |
| **内存安全** | zeroize | 1.8 |
| **Android Native** | Kotlin | 1.9.25 |
| **Android API Level** | min 30 / target 36 | Android 11+ |
| **Java** | JDK | 17 |

---

## 📁 项目结构

```
cameraftp/
├── 📄 配置文件
│   ├── package.json              # Node.js依赖
│   └── build.sh                  # ⭐ 统一构建入口
│
├── 📁 scripts/                   # 构建脚本
│   ├── build-common.sh           # 公共函数库
│   ├── build-windows.sh          # Windows构建
│   ├── build-android.sh          # Android构建
│   └── build-frontend.sh         # 前端构建
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
│   │   ├── lib.rs                # 库入口
│   │   ├── commands/             # Tauri命令（IPC接口）
│   │   │   ├── mod.rs            # 命令模块入口
│   │   │   ├── server.rs         # 服务器控制命令
│   │   │   ├── config.rs         # 配置管理命令
│   │   │   ├── storage.rs        # 存储相关命令
│   │   │   ├── file_index.rs     # 文件索引命令
│   │   │   └── exif.rs           # EXIF读取命令
│   │   ├── ftp/                  # FTP服务器实现
│   │   ├── file_index/           # 文件索引服务
│   │   ├── auto_open/            # 自动预览服务（Windows）
│   │   ├── platform/             # 平台适配（Windows/Android）
│   │   ├── crypto.rs             # Argon2密码哈希
│   │   ├── config.rs             # 应用配置管理
│   │   └── error.rs              # 错误处理
│   │
│   └── 📁 gen/android/           # Android原生代码 (Kotlin)
│       └── app/src/main/java/com/gjk/cameraftpcompanion/
│           ├── MainActivity.kt           # 主活动
│           ├── FtpForegroundService.kt   # FTP前台服务
│           ├── PermissionBridge.kt       # 权限JS Bridge
│           ├── StorageHelper.kt          # 存储辅助
│           ├── MediaScannerHelper.kt     # 媒体扫描
│           └── bridges/                  # JS Bridge 目录
│               ├── FileUploadBridge.kt   # 文件上传Bridge
│               ├── ServerStateBridge.kt  # 服务器状态Bridge
│               └── FileWatcherBridge.kt  # 文件监听Bridge
│
└── 📁 dist/                      # 构建输出
```

---

## 🤖 Android 原生代码

Android平台使用Kotlin实现以下功能：

| 文件 | 功能 |
|------|------|
| **MainActivity.kt** | 主活动，管理应用生命周期和Bridge注册 |
| **FtpForegroundService.kt** | 前台服务，保持FTP在后台运行，显示状态通知 |
| **PermissionBridge.kt** | 权限管理（存储、通知、电池优化） |
| **StorageHelper.kt** | 跳转到系统存储权限设置页面 |
| **MediaScannerHelper.kt** | 文件上传后触发媒体扫描，让照片出现在相册 |
| **bridges/FileUploadBridge.kt** | 文件上传事件处理，触发媒体扫描 |
| **bridges/ServerStateBridge.kt** | 服务器状态更新转发到前台服务 |
| **bridges/FileWatcherBridge.kt** | 使用 FileObserver 监听文件系统变化 |

### JS Bridge 说明

前端通过以下Bridge与Android原生交互：

```typescript
// 存储权限设置
window.StorageSettingsAndroid?.openAllFilesAccessSettings()

// 权限检查
window.PermissionAndroid?.checkAllPermissions()
window.PermissionAndroid?.requestStoragePermission()

// 文件上传事件（由Rust通过Tauri事件触发）
window.FileUploadBridge?.onFileUploaded(path)

// 服务器状态更新
window.ServerStateBridge?.onServerStateChanged(isRunning, statsJson, connectedClients)

// 文件系统监听（Android端使用 FileObserver）
window.FileWatcherBridge?.startWatching(path)
window.FileWatcherBridge?.stopWatching()
window.FileWatcherBridge?.isWatching()
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
./build.sh <target> [options]

# 构建目标：
./build.sh windows         # Windows 可执行文件
./build.sh android         # Android APK (release)
./build.sh frontend        # 仅构建前端

# 构建选项：
./build.sh windows --debug     # Windows Debug 版本
./build.sh android --debug     # Android Debug APK
./build.sh windows --check     # 检查 Windows 编译环境
./build.sh android --check     # 检查 Android 编译环境
./build.sh windows android     # 并行构建多个目标
./build.sh windows --serial    # 串行编译（默认并行）

# 其他命令：
./build.sh gen-types       # 生成 TypeScript 类型绑定
./build.sh clean           # 清理所有构建缓存
```

---

## ⚙️ 配置与存储

### 配置文件位置

- **Windows**: `%APPDATA%\cameraftp\config.json`
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

AGPL-3.0-or-later © 2026 GoldJohnKing <GoldJohnKing@Live.cn>

---

**让摄影工作流更简单！** 📷
