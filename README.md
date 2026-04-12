# 📸 CameraFTP（图传伴侣）

一款跨平台的相机FTP伴侣应用，让相机照片直接传输到电脑或手机。

![版本](https://img.shields.io/badge/version-1.5.0-blue)
![平台](https://img.shields.io/badge/platform-Windows%20%7C%20Android-brightgreen)
![技术栈](https://img.shields.io/badge/tech-Tauri%20v2%20%2B%20React%2018%20%2B%20Rust%202021-orange)

---

## ✨ 功能特性

- 🚀 **一键启动** - 一键启动，无需复杂配置，自动显示连接信息（IP+端口）
- 📡 **FTP服务器** - 基于FTP协议，无需蓝牙，相机WiFi直传
  - 🔐支持加密，FTP/FTPS协议自适应
  - 🔑支持自定义用户名/密码
- 📊 **实时统计** - 实时显示连接状态、最新照片、已接收照片数、数据量
- 🖼️ **自动预览** - 支持接收照片后自动打开预览窗口
- 📷 **EXIF元数据** - 预览窗口支持显示ISO/光圈/快门速度/焦距/拍摄时间
- 🎨 **多格式支持** - 支持 JPG、HEIF、RAW 等格式

### 🖥️ Windows专属功能

- 🔔 **开机自启** - Windows后台运行，支持开机自启
- 🚦 **状态指示** - 托盘图标颜色显示服务器状态
- 🔽 **后台运行** - 支持最小化到系统托盘
- 👁️ **多种预览模式** - 内置预览 / Windows照片查看器 / 系统默认 / 自定义程序

### 📱 Android专属功能

- 🔐 **权限导览** - 缺少必要权限时，显示权限配置导览，一键直达授权界面
- 🚦 **状态指示** - 常驻通知显示服务器状态
- 🛡️ **运行保活** - 前台服务保活，WiFi锁+Wake锁，避免进程意外结束（需ROM支持）
- 🖼️ **内置图库** - 以缩略图形式显示已接收的图片，支持批量选中、删除和分享

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
A: 确保已授权APP"访问全部照片和视频"。

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

# 构建选项
--debug     # Debug 模式
--serial    # 串行构建（默认并行）
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
| **类型生成** | ts-rs | 10.1 |
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
│   ├── App.tsx                   # 主应用组件（三Tab布局）
│   ├── bootstrap/                # 应用启动逻辑
│   │   └── useAppBootstrap.ts    # 启动引导Hook
│   ├── components/               # UI组件
│   │   ├── ui/                   # 基础UI组件
│   │   │   ├── Card.tsx          # 卡片容器
│   │   │   ├── ErrorBoundary.tsx # 错误边界
│   │   │   ├── ErrorMessage.tsx  # 错误显示
│   │   │   ├── IconContainer.tsx # 图标容器
│   │   │   ├── LoadingButton.tsx # 加载按钮
│   │   │   └── ToggleSwitch.tsx  # 开关组件
│   │   ├── ServerCard.tsx        # 服务器控制卡片
│   │   ├── InfoCard.tsx          # 连接信息卡片
│   │   ├── StatsCard.tsx         # 统计信息卡片
│   │   ├── LatestPhotoCard.tsx   # 最新照片卡片
│   │   ├── GalleryCard.tsx       # 图库组件
│   │   ├── VirtualGalleryGrid.tsx # 虚拟滚动图库
│   │   ├── ConfigCard.tsx        # 配置卡片
│   │   ├── AdvancedConnectionConfig.tsx # 高级连接配置
│   │   ├── PreviewConfigCard.tsx # 预览配置
│   │   ├── PreviewWindow.tsx     # 预览窗口
│   │   ├── PathSelector.tsx      # 路径选择器
│   │   ├── BottomNav.tsx         # 底部导航栏
│   │   ├── PermissionDialog.tsx  # 权限对话框
│   │   ├── PermissionList.tsx    # 权限列表
│   │   ├── AboutCard.tsx         # 关于信息
│   │   └── WeChatDonateDialog.tsx # 赞赏对话框
│   ├── hooks/                    # React Hooks
│   │   ├── usePlatform.ts        # 平台检测
│   │   ├── usePortCheck.ts       # 端口检查
│   │   ├── useLatestPhoto.ts     # 最新照片
│   │   ├── useGalleryPager.ts    # 图库分页
│   │   ├── useGallerySelection.ts # 图库多选
│   │   ├── useThumbnailScheduler.ts # 缩略图调度
│   │   ├── useImagePreviewOpener.ts # 图片预览
│   │   ├── useAndroidAutoOpenLatestPhoto.ts # Android自动打开
│   │   ├── usePreviewWindowLifecycle.ts # 预览窗口生命周期
│   │   ├── usePreviewZoomPan.ts  # 预览缩放平移
│   │   ├── usePreviewNavigation.ts # 预览导航
│   │   ├── usePreviewToolbarAutoHide.ts # 工具栏自动隐藏
│   │   ├── usePreviewExif.ts     # 预览EXIF数据
│   │   ├── usePreviewConfigListener.ts # 预览配置监听
│   │   └── useQuitFlow.ts       # 退出流程
│   ├── services/                 # 业务逻辑服务
│   │   ├── server-events.ts      # 服务器事件处理
│   │   ├── gallery-media-v2.ts   # 图库媒体服务V2
│   │   ├── latest-photo.ts       # 最新照片服务
│   │   └── image-open.ts         # 图片打开服务
│   ├── stores/                   # Zustand状态管理
│   │   ├── serverStore.ts        # 服务器状态
│   │   ├── configStore.ts        # 配置状态（防抖自动保存）
│   │   └── permissionStore.ts    # 权限状态（Android）
│   ├── types/                    # TypeScript类型定义
│   │   ├── index.ts              # 类型导出（ts-rs生成）
│   │   ├── gallery-v2.ts         # 图库类型
│   │   ├── events.ts             # 事件类型
│   │   └── global.ts             # 全局类型声明
│   └── utils/                    # 工具函数
│       ├── events.ts             # 事件管理器
│       ├── format.ts             # 格式化工具
│       ├── error.ts              # 错误处理
│       ├── gallery-refresh.ts    # 图库刷新
│       ├── gallery-delete.ts     # 图库删除
│       └── store.ts              # 异步Store辅助
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
│   │   │   ├── storage.rs        # 存储/权限/自启命令
│   │   │   ├── file_index.rs     # 文件索引命令
│   │   │   └── exif.rs           # EXIF读取命令
│   │   ├── ftp/                  # FTP服务器实现
│   │   │   ├── server.rs         # FtpServerActor（生命周期管理）
│   │   │   ├── server_factory.rs # 启动流水线
│   │   │   ├── events.rs         # EventBus + 事件处理器
│   │   │   ├── listeners.rs      # FTP数据/连接事件监听
│   │   │   ├── stats.rs          # StatsActor（统计聚合）
│   │   │   ├── types.rs          # 类型定义
│   │   │   └── android_mediastore/ # Android MediaStore后端
│   │   │       ├── backend.rs    # StorageBackend实现
│   │   │       ├── bridge.rs     # JNI桥接
│   │   │       ├── types.rs      # 数据类型
│   │   │       ├── retry.rs      # 重试逻辑
│   │   │       └── limiter.rs    # 并发限制
│   │   ├── file_index/           # 文件索引服务
│   │   │   ├── service.rs        # 索引服务（EXIF排序）
│   │   │   ├── types.rs          # 索引类型
│   │   │   └── watcher.rs        # 文件监听（Windows）
│   │   ├── auto_open/            # 自动预览服务
│   │   │   ├── service.rs        # 预览路由
│   │   │   └── windows.rs        # Windows预览实现
│   │   ├── platform/             # 平台适配层
│   │   │   ├── traits.rs         # PlatformService接口
│   │   │   ├── types.rs          # 平台类型
│   │   │   ├── windows.rs        # Windows实现（托盘/自启）
│   │   │   └── android.rs        # Android实现（JNI/权限）
│   │   ├── crypto/               # 加密模块
│   │   │   └── tls.rs            # TLS证书生成/轮换
│   │   ├── utils/                # 工具模块
│   │   │   └── fs.rs             # 文件系统工具
│   │   ├── config.rs             # 配置类型定义
│   │   ├── config_service.rs     # 配置服务
│   │   ├── crypto.rs             # Argon2id密码哈希
│   │   ├── network.rs            # 网络接口检测
│   │   ├── constants.rs          # 应用常量
│   │   └── error.rs              # 错误处理
│   │
│   └── 📁 gen/android/           # Android原生代码 (Kotlin)
│       └── app/src/main/java/com/gjk/cameraftpcompanion/
│           ├── MainActivity.kt                    # 主活动，WebView管理和Bridge注册
│           ├── FtpForegroundService.kt            # 前台服务，WiFi锁+Wake锁
│           ├── PermissionBridge.kt                # 权限管理Bridge
│           ├── ImageViewerActivity.kt             # 全屏图片查看Activity
│           ├── ImageViewerAdapter.kt              # 图片查看适配器
│           ├── AndroidServiceStateCoordinator.kt  # 服务状态协调（Rust↔Android）
│           ├── bridges/                           # JS Bridge目录
│           │   ├── BaseJsBridge.kt                # Bridge基类
│           │   ├── GalleryBridge.kt               # 原始图库Bridge
│           │   ├── GalleryBridgeV2.kt             # 增强图库Bridge（分页+缓存）
│           │   ├── ImageViewerBridge.kt           # 图片查看Bridge
│           │   └── MediaStoreBridge.kt            # MediaStore集成Bridge
│           └── galleryv2/                         # Gallery V2实现
│               ├── MediaPageProvider.kt           # 分页媒体加载
│               ├── ThumbnailCacheV2.kt            # 缩略图缓存（内存+磁盘）
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
| **MainActivity.kt** | 主活动，WebView管理和所有Bridge注册 |
| **FtpForegroundService.kt** | 前台服务，WiFi锁+Wake锁，状态通知 |
| **PermissionBridge.kt** | 权限管理（存储、通知、电池优化） |
| **ImageViewerActivity.kt** | 全屏图片查看（ViewPager2 + 捏合缩放 + EXIF叠加） |
| **ImageViewerAdapter.kt** | 图片查看适配器 |
| **AndroidServiceStateCoordinator.kt** | 服务状态协调（Rust↔Android JNI同步） |
| **bridges/GalleryBridge.kt** | 原始图库Bridge |
| **bridges/GalleryBridgeV2.kt** | 增强图库Bridge，支持分页加载和缩略图缓存 |
| **bridges/ImageViewerBridge.kt** | 图片查看Bridge，支持全屏查看和EXIF回调 |
| **bridges/MediaStoreBridge.kt** | MediaStore集成Bridge，供Kotlin/Rust集成调用 |
| **galleryv2/MediaPageProvider.kt** | 分页媒体加载 |
| **galleryv2/ThumbnailCacheV2.kt** | 缩略图缓存，内存+磁盘两级 |
| **galleryv2/ThumbnailDecoder.kt** | 缩略图解码 |
| **galleryv2/ThumbnailKeyV2.kt** | 缓存键 |
| **galleryv2/ThumbnailPipelineManager.kt** | 缩略图管道管理 |

#### JS Bridge 说明

前端通过以下Bridge与Android原生交互：

```typescript
// 权限管理
window.PermissionAndroid?.checkAll()
window.PermissionAndroid?.requestStorage()
window.PermissionAndroid?.requestNotification()
window.PermissionAndroid?.requestBatteryOptimization()

// 图库访问（V2增强版，支持分页和缓存）
window.GalleryAndroidV2?.listMediaPage(requestJson)
window.GalleryAndroidV2?.enqueueThumbnails(requestsJson)
window.GalleryAndroidV2?.registerThumbnailListener(viewId, listenerId)
window.GalleryAndroidV2?.cancelThumbnailRequests(idsJson)

// 图片查看
window.ImageViewerAndroid?.openOrNavigateTo(uri, allUrisJson)
```

</details>
