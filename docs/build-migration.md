# Android 构建系统迁移指南

## 概述

从 v2.0 开始，Android 构建支持 Windows 和 Linux 混合工具链。
每个构建工具（cargo, java, SDK）都可以独立选择 Windows 或 Linux 版本。

## 工具选择优先级

每个工具按以下顺序检测：

1. **Windows 版本** (`.exe`) - 优先
2. **Linux 版本** - 回退

### 支持的工具

| 工具    | Windows 命令  | Linux 命令   | 必需性  |
| ------- | ------------- | ------------ | ------- |
| Cargo   | `cargo.exe`   | `cargo`      | ✅ 必需 |
| Java    | `java.exe`    | `java`       | ✅ 必需 |
| Javac   | `javac.exe`   | `javac`      | ✅ 必需 |
| Keytool | `keytool.exe` | `keytool`    | ⚠️ 可选 |
| ADB     | `adb.exe`     | `adb`        | ⚠️ 可选 |

### 路径检测优先级

#### Android SDK

**Windows 路径** (WSL 中):
- `/mnt/c/Users/<user>/AppData/Local/Android/Sdk`
- `/mnt/c/Users/<user>/AppData/Roaming/Android/Sdk`

**Linux 路径**:
- `$HOME/Android/Sdk`
- `$ANDROID_HOME`
- `$ANDROID_SDK_ROOT`

#### Java Home

**Windows 路径**:
- `/mnt/c/Program Files/Java/*`
- `/mnt/c/Program Files/Eclipse Adoptium/*`

**Linux 路径**:
- `/usr/lib/jvm/java-21-openjdk-amd64`
- `/usr/lib/jvm/java-17-openjdk-amd64`
- `$JAVA_HOME`

## 使用场景

### 场景 1: 纯 Windows 环境 (推荐)

所有工具都使用 Windows 版本：
- cargo.exe
- java.exe
- C:\\Users\\<user>\\AppData\\Local\\Android\\Sdk

### 场景 2: 纯 Linux 环境

所有工具都使用 Linux 版本：
- cargo
- java
- ~/Android/Sdk

### 场景 3: 混合环境 (WSL)

混合使用 Windows 和 Linux 工具：
- cargo.exe (Windows - 编译更快)
- java (Linux - Android SDK 在 Linux)
- ~/Android/Sdk (Linux)

⚠️ **警告**: 混合模式可能遇到兼容性问题，确保版本兼容。

## 检查当前配置

```bash
./build.sh android --check
```

输出示例：
```
[INFO] 检查 Android 编译环境 (支持 Windows/Linux 混合)...
[INFO] Bun: 1.x.x
[INFO] Cargo: cargo 1.x.x (Windows)
  → 使用 windows 版本的 Cargo
[INFO] Java: openjdk version "21.x.x" (Linux)
  → 使用 linux 版本的 Java
[INFO] Android SDK: /home/user/Android/Sdk (Linux)
[INFO] Android NDK: /home/user/Android/Sdk/ndk/26.x.x
[WARN] 检测到混合工具链模式 (Windows + Linux)
[WARN] 这在 WSL 环境中常见，但可能遇到兼容性问题
[SUCCESS] Android 环境检查通过
```

## 故障排除

### 工具未找到

如果某个工具被错误地检测或未被检测：

1. 检查 PATH 环境变量
2. 确保工具在 PATH 中
3. 使用完整路径创建符号链接

### 混合模式兼容性问题

如果遇到兼容性错误：

1. 统一使用同一平台的工具链
2. 或者确保版本兼容（如 Rust 目标三元组匹配）

### 调试工具选择

```bash
./scripts/test-tool-selection.sh
```

## 回退到旧行为

如果需要强制使用特定平台：

```bash
# 强制 Linux 模式
unset cargo.exe  # 从 PATH 移除 Windows cargo
./build.sh android

# 强制 Windows 模式  
unset cargo      # 从 PATH 移除 Linux cargo
./build.sh android
```
