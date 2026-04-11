# 📸 CameraFTP（图传伴侣）

一款跨平台的相机FTP伴侣应用，让相机照片直接传输到电脑或手机。

![版本](https://img.shields.io/badge/version-1.4.0-blue)
![平台](https://img.shields.io/badge/platform-Windows%20%7C%20Android-brightgreen)
![技术栈](https://img.shields.io/badge/tech-Tauri%20v2%20%2B%20React%2018%20%2B%20Rust%202021-orange)

---

## ✨ 功能特性

- 🚀 **一键启动** - 一键启动，无需复杂配置，自动显示连接信息（IP+端口）
- 📡 **FTP服务器** - 基于FTP协议，无需蓝牙，相机WiFi直传
  - 🔐支持加密，FTP/FTPS协议自适应
  - 🔑支持自定义用户名/密码（非匿名模式）
- 📊 **实时统计** - 实时显示连接状态、最新照片、已接收照片数、数据量
- 🖼️ **自动预览** - 支持接收照片后自动打开预览窗口
- 📷 **EXIF元数据** - 预览窗口支持显示ISO/光圈/快门速度/拍摄时间
- 🎨 **多格式支持** - 支持 JPG 和 HEIF 等格式

### 🖥️ Windows专属功能

- 🔔 **开机自启** - Windows后台运行，支持开机自启
- 🚦 **状态指示** - 托盘图标颜色显示服务器状态

### 📱 Android专属功能

- 🔐 **权限导览** - 缺少必要权限时，显示权限配置导览，一键直达授权界面
- 🚦 **状态指示** - 常驻通知显示服务器状态
- 🛡️ **运行保活** - 前台服务保活，避免进程意外结束（需ROM支持）

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
A: 确保电脑和相机在同一网络，并检查防火墙设置。

**Q: Android无法保存照片？**  
A: 确保已授权APP“访问全部照片和视频”。

---

## 💰 支持项目

如果这个项目对你有帮助，欢迎打赏支持开发！

<img src="docs/images/wechat_alipay.png" alt="微信支付 / 支付宝" width="300"/>

---

## 📄 许可证

AGPL-3.0-or-later © 2026 GoldJohnKing <GoldJohnKing@Live.cn>

---

<details>
<summary><h2>🛠️ 开发指南</h2></summary>

### 🚀 一键编译

```bash
# 统一构建入口
./build.sh <target> [options]

# 构建目标
./build.sh windows          # Windows 可执行文件 (Release)
./build.sh android          # Android APK (Release)
./build.sh frontend         # 仅构建前端
./build.sh windows android  # 并行构建

# 其他命令
./build.sh gen-types                # 生成 TypeScript 类型绑定
./build.sh clean                    # 清理所有构建缓存
./build.sh windows android --check  # 检查编译环境
```

---

### 🏗️ 技术架构

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
| **前端语言** | TypeScript | ^5.0.2 |
| **状态管理** | Zustand | ^5.0.11 |
| **UI组件** | lucide-react | ^0.460.0 |
| **Toast通知** | sonner | ^2.0.7 |
| **样式** | TailwindCSS | ^3.4.15 |
| **构建工具** | Vite | ^5.0.0 |
| **后端语言** | Rust | Edition 2021 |
| **异步运行时** | tokio | ^1.0 |
| **FTP服务器** | libunftp | 0.23.0 |
| **FTP存储后端** | unftp-sbe-fs | 0.4.0 |
| **类型生成** | ts-rs | ^10.1 |
| **EXIF读取** | nom-exif | 2.7 |
| **时间处理** | chrono | 0.4 |
| **托盘图标** | image | 0.25 |
| **密码哈希** | argon2 | 0.5 |
| **内存安全** | zeroize | 1.8 |
| **TLS证书** | rcgen | 0.13 |
| **并发集合** | dashmap | 6.0 |
| **错误处理** | thiserror | 2.0 |
| **日志** | tracing | 0.1 |
| **文件监听(Win)** | notify | 8.0 |
| **Android Native** | Kotlin | JVM 21 |
| **Android API** | min 33 / target 36 | Android 13+ |
| **JDK** | Java | 21 |

---

### 📁 项目结构

```
cameraftp/
├── 📄 配置文件
│   ├── package.json              # Node.js依赖
│   ├── tsconfig.json             # TypeScript配置
│   ├── vite.config.ts            # Vite配置
│   ├── tailwind.config.js        # TailwindCSS配置
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
│   ├── bootstrap/                # 应用启动逻辑
│   │   └── useAppBootstrap.ts    # 启动引导Hook
│   ├── components/               # UI组件
│   │   ├── ui/                   # 基础UI组件
│   │   ├── ServerCard.tsx        # 服务器控制卡片
│   │   ├── ConfigCard.tsx        # 配置卡片
│   │   ├── StatsCard.tsx         # 统计信息卡片
│   │   ├── LatestPhotoCard.tsx   # 最新照片卡片
│   │   ├── GalleryCard.tsx       # 图库组件
│   │   ├── VirtualGalleryGrid.tsx # 虚拟滚动图库
│   │   ├── PreviewWindow.tsx     # 预览窗口
│   │   └── ...                   # 其他组件
│   ├── hooks/                    # React Hooks
│   │   ├── useThumbnailScheduler.ts
│   │   ├── usePreviewNavigation.ts
│   │   ├── useGalleryPager.ts
│   │   └── ...                   # 其他Hooks
│   ├── services/                 # 业务逻辑服务
│   │   ├── server-events.ts      # 服务器事件处理
│   │   ├── gallery-media-v2.ts   # 图库媒体服务
│   │   ├── latest-photo.ts       # 最新照片服务
│   │   └── ...
│   ├── stores/                   # Zustand状态管理
│   │   ├── serverStore.ts        # 服务器状态
│   │   ├── configStore.ts        # 配置状态
│   │   └── permissionStore.ts    # 权限状态
│   ├── types/                    # TypeScript类型定义
│   │   ├── index.ts              # 类型导出
│   │   ├── gallery-v2.ts         # 图库类型
│   │   └── events.ts             # 事件类型
│   └── utils/                    # 工具函数
│       ├── format.ts             # 格式化工具
│       ├── gallery-refresh.ts    # 图库刷新
│       └── ...
│
├── 📁 src-tauri/                 # Rust后端源码
│   ├── Cargo.toml                # Rust依赖
│   ├── build.rs                  # 构建脚本
│   ├── src/
│   │   ├── main.rs               # 程序入口
│   │   ├── lib.rs                # 库入口 & Tauri命令注册
│   │   ├── commands/             # Tauri命令（IPC接口）
│   │   │   ├── mod.rs            # 命令模块入口
│   │   │   ├── server.rs         # 服务器控制命令
│   │   │   ├── config.rs         # 配置管理命令
│   │   │   ├── storage.rs        # 存储相关命令
│   │   │   ├── file_index.rs     # 文件索引命令
│   │   │   └── exif.rs           # EXIF读取命令
│   │   ├── ftp/                  # FTP服务器实现
│   │   │   ├── server.rs         # FTP服务器核心
│   │   │   ├── events.rs         # FTP事件处理
│   │   │   ├── listeners.rs      # 连接监听器
│   │   │   ├── stats.rs          # 统计信息
│   │   │   ├── types.rs          # FTP类型定义
│   │   │   └── android_mediastore/ # Android媒体库集成
│   │   ├── file_index/           # 文件索引服务
│   │   │   ├── service.rs        # 索引服务
│   │   │   ├── types.rs          # 索引类型
│   │   │   └── watcher.rs        # 文件监听
│   │   ├── auto_open/            # 自动预览服务（Windows）
│   │   ├── platform/             # 平台适配层
│   │   │   ├── mod.rs            # 平台入口
│   │   │   ├── traits.rs         # 平台接口定义
│   │   │   ├── types.rs          # 平台类型
│   │   │   ├── windows.rs        # Windows实现
│   │   │   └── android.rs        # Android实现
│   │   ├── utils/                # 工具模块
│   │   ├── constants.rs          # 应用常量
│   │   ├── config.rs             # 配置管理
│   │   ├── config_service.rs     # 配置服务
│   │   ├── crypto.rs             # 密码哈希(TLS/Argon2)
│   │   ├── network.rs            # 网络工具
│   │   └── error.rs              # 错误处理
│   │
│   └── 📁 gen/android/           # Android原生代码 (Kotlin)
│       └── app/src/main/java/com/gjk/cameraftpcompanion/
│           ├── MainActivity.kt                    # 主活动，WebView管理和Bridge注册
│           ├── FtpForegroundService.kt            # 前台服务，后台运行FTP
│           ├── PermissionBridge.kt                # 权限管理Bridge
│           ├── ImageViewerActivity.kt             # 全屏图片查看
│           ├── ImageViewerAdapter.kt              # 图片查看适配器
│           ├── AndroidServiceStateCoordinator.kt  # 服务状态协调
│           ├── bridges/                           # JS Bridge目录
│           │   ├── BaseJsBridge.kt                # Bridge基类
│           │   ├── GalleryBridge.kt               # 原始图库Bridge
│           │   ├── GalleryBridgeV2.kt             # 增强图库Bridge(分页)
│           │   ├── ImageViewerBridge.kt           # 图片查看Bridge
│           │   └── MediaStoreBridge.kt            # MediaStore访问Bridge
│           └── galleryv2/                         # Gallery V2实现
│               ├── MediaPageProvider.kt           # 分页媒体加载
│               ├── ThumbnailCacheV2.kt            # 缩略图缓存
│               ├── ThumbnailDecoder.kt            # 缩略图解码
│               ├── ThumbnailKeyV2.kt              # 缓存键
│               └── ThumbnailPipelineManager.kt    # 缩略图管道管理
│
└── 📁 dist/                      # 构建输出

---

### 🤖 Android 原生代码

Android平台使用Kotlin实现以下功能：

| 文件 | 功能 |
|------|------|
| **MainActivity.kt** | 主活动，WebView管理和Bridge注册 |
| **FtpForegroundService.kt** | 前台服务，后台运行FTP，显示状态通知 |
| **PermissionBridge.kt** | 权限管理（存储、通知、电池优化） |
| **ImageViewerActivity.kt** | 全屏图片查看Activity |
| **ImageViewerAdapter.kt** | 图片查看适配器 |
| **AndroidServiceStateCoordinator.kt** | 服务状态协调 |
| **bridges/GalleryBridge.kt** | 原始图库Bridge，提供基础图库访问 |
| **bridges/GalleryBridgeV2.kt** | 增强图库Bridge，支持分页加载和缩略图缓存 |
| **bridges/ImageViewerBridge.kt** | 图片查看Bridge，支持全屏查看和分享 |
| **bridges/MediaStoreBridge.kt** | MediaStore 原生访问与上传落库辅助，供 Kotlin/Rust 集成调用 |
| **galleryv2/MediaPageProvider.kt** | 分页媒体加载，高效加载大量图片 |
| **galleryv2/ThumbnailCacheV2.kt** | 缩略图缓存，内存+磁盘两级缓存 |
| **galleryv2/ThumbnailDecoder.kt** | 缩略图解码，支持多种图片格式 |
| **galleryv2/ThumbnailKeyV2.kt** | 缓存键，唯一标识缩略图 |
| **galleryv2/ThumbnailPipelineManager.kt** | 缩略图管道管理，协调解码和缓存 |

#### JS Bridge 说明

前端通过以下Bridge与Android原生交互：

```typescript
// 权限管理
window.PermissionAndroid?.checkAllPermissions()
window.PermissionAndroid?.requestStoragePermission()
window.PermissionAndroid?.requestNotificationPermission()

// 图库访问（原始版本）
window.GalleryAndroid?.deleteImages(urisJson)
window.GalleryAndroid?.shareImages(urisJson)

// 图库访问（V2增强版，支持分页和缓存）
window.GalleryAndroidV2?.listMediaPage(requestJson)
window.GalleryAndroidV2?.enqueueThumbnails(requestsJson)
window.GalleryAndroidV2?.registerThumbnailListener(viewId, listenerId)

// 图片查看
window.ImageViewerAndroid?.openOrNavigateTo(uri, allUrisJson)
window.ImageViewerAndroid?.closeViewer()
```

`MediaStoreBridge.kt` 当前不作为 WebView 暴露的 JS Bridge 注册，而是作为 Android 原生侧的 MediaStore/JNI 集成辅助模块使用。

</details>
