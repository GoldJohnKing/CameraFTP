# Android 环境设置指南 (WSL)

本指南介绍如何在 WSL (Windows Subsystem for Linux) 中设置 Android 开发环境。

## 前置要求

- Windows 11 或 Windows 10 (Build 19041+)
- 已安装 WSL2
- 已安装 Ubuntu 或其他 Linux 发行版

## 1. 安装 Java

```bash
# 更新包列表
sudo apt update

# 安装 OpenJDK 17
sudo apt install -y openjdk-17-jdk

# 验证安装
java -version
```

## 2. 安装 Android SDK

### 方法一：使用 Android Studio（推荐）

1. 下载 Android Studio：
   ```bash
   wget https://redirector.gvt1.com/edgedl/android/studio/ide-zips/2023.1.1.28/android-studio-2023.1.1.28-linux.tar.gz
   ```

2. 解压到主目录：
   ```bash
   tar -xzf android-studio-*.tar.gz -C ~/
   ```

3. 运行 Android Studio 设置向导：
   ```bash
   ~/android-studio/bin/studio.sh
   ```

### 方法二：命令行工具（轻量级）

```bash
# 创建 SDK 目录
mkdir -p ~/Android/Sdk
cd ~/Android/Sdk

# 下载命令行工具
wget https://dl.google.com/android/repository/commandlinetools-linux-11076708_latest.zip

# 解压
unzip commandlinetools-linux-*.zip
mkdir -p cmdline-tools/latest
mv cmdline-tools/* cmdline-tools/latest/ 2>/dev/null || true

# 安装必要组件
yes | cmdline-tools/latest/bin/sdkmanager --licenses
cmdline-tools/latest/bin/sdkmanager "platforms;android-34"
cmdline-tools/latest/bin/sdkmanager "build-tools;34.0.0"
cmdline-tools/latest/bin/sdkmanager "platform-tools"
```

## 3. 配置环境变量

编辑 `~/.bashrc` 或 `~/.zshrc`：

```bash
# Java
export JAVA_HOME=/usr/lib/jvm/java-17-openjdk-amd64
export PATH=$PATH:$JAVA_HOME/bin

# Android SDK
export ANDROID_HOME=$HOME/Android/Sdk
export PATH=$PATH:$ANDROID_HOME/platform-tools
export PATH=$PATH:$ANDROID_HOME/cmdline-tools/latest/bin

# Android Studio（如使用）
export PATH=$PATH:$HOME/android-studio/bin
```

使配置生效：
```bash
source ~/.bashrc  # 或 source ~/.zshrc
```

## 4. 验证安装

```bash
# 检查 adb
adb --version

# 检查 sdkmanager
sdkmanager --list | head -20

# 检查环境变量
echo $ANDROID_HOME
echo $JAVA_HOME
```

## 5. 安装 Rust Android 目标

```bash
rustup target add aarch64-linux-android
rustup target add armv7-linux-androideabi
rustup target add i686-linux-android
rustup target add x86_64-linux-android
```

## 6. 连接 Android 设备

### 方法一：USB 连接（需要 Windows 端 adb）

1. 在 Windows 上安装 adb：
   - 下载 [Platform Tools](https://developer.android.com/studio/releases/platform-tools)
   - 解压并添加到 PATH

2. 在 Windows PowerShell 中启动 adb 服务器：
   ```powershell
   adb -a -P 5037 nodaemon server
   ```

3. 在 WSL 中连接：
   ```bash
   export ADB_SERVER_SOCKET=tcp:$(cat /etc/resolv.conf | grep nameserver | awk '{print $2}'):5037
   adb devices
   ```

### 方法二：WiFi 连接

1. 在设备上启用开发者选项和 WiFi 调试
2. 在 WSL 中连接：
   ```bash
   adb connect 设备IP:5555
   adb devices
   ```

### 方法三：使用 Android 模拟器

在 Windows 上运行 Android Studio 模拟器，然后在 WSL 中连接：

```bash
# 设置 ADB 服务器地址
export ADB_SERVER_SOCKET=tcp:$(cat /etc/resolv.conf | grep nameserver | awk '{print $2}'):5037
adb devices
```

## 7. 快速测试

```bash
# 进入项目目录
cd /mnt/d/GitRepos/camera-ftp-companion

# 检查环境
./build-android.sh check

# 列出已连接设备
./build-android.sh devices

# 构建 Debug APK
./build-android.sh debug
```

## 常见问题

### Q: adb 无法找到设备
**A**: 确保 Windows 端的 adb 服务器正在运行，并设置 ADB_SERVER_SOCKET 环境变量。

### Q: Gradle 构建缓慢
**A**: 创建 `~/.gradle/gradle.properties`：
```properties
org.gradle.jvmargs=-Xmx4096m
org.gradle.daemon=true
org.gradle.parallel=true
org.gradle.configureondemand=true
```

### Q: 内存不足
**A**: 在 WSL 配置中增加内存限制。创建或编辑 `%USERPROFILE%\.wslconfig`：
```ini
[wsl2]
memory=8GB
processors=4
```

然后重启 WSL：
```powershell
wsl --shutdown
```

### Q: 文件系统权限问题
**A**: WSL 挂载的 Windows 磁盘可能有权限问题。确保脚本有执行权限：
```bash
chmod +x build-android.sh
```

## 下一步

环境设置完成后，参考 [Android README](./src-tauri/gen/android/README.md) 进行应用构建和开发。
