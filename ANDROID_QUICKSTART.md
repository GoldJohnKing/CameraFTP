# Android 版本快速开始

本文档帮助您快速构建图传伴侣的 Android 版本。

## 项目结构更新

已创建的文件和目录：

```
camera-ftp-companion/
├── build-android.sh                    # Android 编译脚本 ⭐
├── ANDROID_SETUP.md                    # WSL 环境设置指南
├── package.json                        # 添加 Android npm 脚本
├── src-tauri/
│   ├── tauri.conf.json                 # 添加 Android 支持配置
│   └── gen/android/                    # Android 项目目录
│       ├── README.md                   # Android 项目文档
│       ├── build.gradle.kts
│       ├── settings.gradle.kts
│       ├── gradle.properties
│       └── app/
│           ├── build.gradle.kts
│           ├── proguard-rules.pro
│           ├── tauri.build.gradle.kts
│           └── src/main/
│               ├── AndroidManifest.xml
│               ├── java/com/cameraftp/companion/MainActivity.kt
│               └── res/
│                   ├── values/
│                   │   ├── colors.xml
│                   │   ├── strings.xml
│                   │   └── themes.xml
│                   ├── drawable/
│                   │   └── splash_background.xml
│                   └── xml/
│                       └── network_security_config.xml
└── src-tauri/src/platform/
    └── android.rs                      # 更新 Android 平台适配
```

## 快速构建（三步完成）

### 第一步：环境检查

```bash
./build-android.sh check
```

如果环境未就绪，请参考 [ANDROID_SETUP.md](./ANDROID_SETUP.md) 完成设置。

### 第二步：开发模式（可选）

在真实设备或模拟器上测试：

```bash
./build-android.sh dev
```

### 第三步：构建 APK

#### Debug 版本（测试用）

```bash
./build-android.sh debug
```

输出位置：`src-tauri/gen/android/app/build/outputs/apk/debug/app-debug.apk`

#### Release 版本（发布用）

```bash
./build-android.sh release
```

#### Google Play 上架（AAB 格式）

```bash
./build-android.sh aab
```

## 常用命令

| 命令 | 说明 |
|------|------|
| `./build-android.sh check` | 检查编译环境 |
| `./build-android.sh init` | 初始化 Android 项目 |
| `./build-android.sh dev` | 开发模式（热重载） |
| `./build-android.sh debug` | 构建 Debug APK |
| `./build-android.sh release` | 构建 Release APK |
| `./build-android.sh aab` | 构建 AAB 包 |
| `./build-android.sh devices` | 列出已连接设备 |
| `./build-android.sh install` | 安装 APK 到设备 |
| `./build-android.sh studio` | 在 Android Studio 中打开 |
| `./build-android.sh help` | 显示帮助 |

或者使用 npm 脚本：

```bash
bun run android:dev      # 开发模式
bun run android:build    # 构建 Release
bun run android:studio   # 打开 Android Studio
```

## 重要配置

### 应用签名（Release 必需）

1. 生成密钥库：
   ```bash
   keytool -genkey -v \
     -keystore my-release-key.keystore \
     -alias cameraftp \
     -keyalg RSA \
     -keysize 2048 \
     -validity 10000
   ```

2. 创建配置文件 `src-tauri/gen/android/keystore.properties`：
   ```properties
   storeFile=my-release-key.keystore
   storePassword=你的密码
   keyAlias=cameraftp
   keyPassword=你的密码
   ```

### 应用图标

需要为 Android 准备不同尺寸的图标，放置在：

```
src-tauri/gen/android/app/src/main/res/
├── mipmap-mdpi/     (48x48)
├── mipmap-hdpi/     (72x72)
├── mipmap-xhdpi/    (96x96)
├── mipmap-xxhdpi/   (144x144)
└── mipmap-xxxhdpi/  (192x192)
```

详情请参考 `src-tauri/gen/android/app/src/main/res/ICONS.md`

## 权限说明

Android 版本申请以下权限：

- **网络权限** - FTP 服务器通信
- **存储权限** - 保存接收的照片
- **前台服务** - 后台运行 FTP 服务器
- **通知权限** - 显示服务状态通知
- **唤醒锁定** - 防止传输中断

## 与桌面版的差异

| 功能 | Windows | Android |
|------|---------|---------|
| 系统托盘 | ✅ | ❌ 使用通知栏 |
| 开机自启 | ✅ | ❌ |
| 后台服务 | ✅ 自动 | ✅ 前台服务 |
| 窗口控制 | ✅ 最小化到托盘 | ✅ 返回后台 |
| 配置存储 | 文件系统 | 应用私有目录 |

## 故障排除

### 构建失败：找不到 Java

```bash
export JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64
export PATH=$PATH:$JAVA_HOME/bin
```

### 构建失败：找不到 Android SDK

```bash
export ANDROID_HOME=$HOME/Android/Sdk
export PATH=$PATH:$ANDROID_HOME/platform-tools
```

### 设备未连接

USB 连接需要 Windows 端 adb 服务器：

```powershell
# Windows PowerShell
adb -a -P 5037 nodaemon server
```

```bash
# WSL
export ADB_SERVER_SOCKET=tcp:$(cat /etc/resolv.conf | grep nameserver | awk '{print $2}'):5037
adb devices
```

WiFi 调试：

```bash
adb connect 设备IP:5555
```

## 下一步

1. 完成环境设置（参考 [ANDROID_SETUP.md](./ANDROID_SETUP.md)）
2. 准备应用图标（参考 ICONS.md）
3. 运行 `./build-android.sh debug` 进行首次构建
4. 在设备上测试功能
5. 配置签名并构建 Release 版本

## 获取帮助

- Tauri 移动端文档：https://tauri.app/start/migrate/from-tauri-1/
- Android 开发者文档：https://developer.android.com/guide
- 项目 Android 文档：`src-tauri/gen/android/README.md`
