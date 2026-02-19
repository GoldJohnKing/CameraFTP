# 开发环境设置指南

## Windows开发环境

### 1. 安装Rust

下载并运行Rust安装程序：
```powershell
# 使用winget
winget install Rustlang.Rustup

# 或使用官方安装器
# https://rustup.rs/
```

安装完成后重启终端，验证：
```bash
rustc --version
cargo --version
```

### 2. 安装Node.js

```powershell
# 使用winget
winget install OpenJS.NodeJS

# 或使用nvm-windows
# https://github.com/coreybutler/nvm-windows
```

### 3. 安装Bun

```powershell
powershell -c "irm bun.sh/install.ps1 | iex"
```

### 4. 安装Tauri CLI

```bash
cargo install tauri-cli
```

### 5. 安装Visual Studio Build Tools

下载并安装 **Visual Studio Build Tools 2022**，选择：
- 使用C++的桌面开发
- Windows 11 SDK (或Windows 10 SDK)

或者使用winget：
```powershell
winget install Microsoft.VisualStudio.2022.BuildTools
```

### 6. 克隆并运行项目

```bash
git clone <repository-url>
cd camera-ftp-companion
bun install --no-cache
cargo tauri dev
```

---

## WSL开发环境（跨平台构建）

### 1. 安装WSL

```powershell
wsl --install
```

### 2. WSL内安装依赖

```bash
# 更新包管理器
sudo apt update

# 安装基础工具
sudo apt install -y curl git build-essential pkg-config

# 安装Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# 安装Bun
curl -fsSL https://bun.sh/install | bash

# 安装Node.js（可选，Bun可替代）
curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash -
sudo apt install -y nodejs
```

### 3. 配置Windows Rust访问

在WSL中使用Windows的Rust工具链构建Windows EXE：

```bash
# 设置Windows cargo路径
export WINDOWS_CARGO="/mnt/c/Users/$USER/.cargo/bin/cargo.exe"

# 验证可访问
$WINDOWS_CARGO --version
```

### 4. 项目设置

```bash
git clone <repository-url>
cd camera-ftp-companion
bun install --no-cache
```

### 5. 构建Windows EXE

```bash
# 使用提供的脚本
./build-full.sh

# 或手动构建
bun run build
cd src-tauri
$WINDOWS_CARGO build --release --target x86_64-pc-windows-msvc
```

---

## Linux开发环境

### Ubuntu/Debian

```bash
# 安装系统依赖
sudo apt update
sudo apt install -y libgtk-3-dev libwebkit2gtk-4.0-dev libappindicator3-dev librsvg2-dev patchelf pkg-config

# 安装Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# 安装Bun
curl -fsSL https://bun.sh/install | bash

# 安装Tauri CLI
cargo install tauri-cli

# 克隆并运行项目
git clone <repository-url>
cd camera-ftp-companion
bun install --no-cache
cargo tauri dev
```

### Fedora

```bash
sudo dnf check-update
sudo dnf install webkit2gtk3-devel.x86_64 openssl-devel curl wget libappindicator-gtk3 librsvg2-devel
cargo install tauri-cli
```

### Arch Linux

```bash
sudo pacman -S webkit2gtk base-devel curl wget openssl appmenu-gtk-module libappindicator-gtk3 librsvg
cargo install tauri-cli
```

---

## Android开发环境

### 1. 安装Android Studio

下载并安装 [Android Studio](https://developer.android.com/studio)

### 2. 安装NDK

在Android Studio中：
- 打开 SDK Manager
- 选择 "SDK Tools"
- 勾选 "NDK (Side by side)"
- 点击Apply安装

### 3. 配置环境变量

```bash
# 添加到 ~/.bashrc 或 ~/.zshrc
export ANDROID_HOME=$HOME/Android/Sdk
export NDK_HOME=$ANDROID_HOME/ndk/25.0.8775105  # 替换为实际版本
export PATH=$PATH:$ANDROID_HOME/platform-tools
```

### 4. 构建Android APK

```bash
# 初始化Android项目
cargo tauri android init

# 开发模式
cargo tauri android dev

# 生产构建
cargo tauri android build
```

---

## 环境验证

运行以下命令验证环境配置：

```bash
# Rust
rustc --version  # 应显示 ≥1.75
cargo --version

# Node.js/Bun
bun --version    # 应显示 ≥1.0
node --version   # 可选

# Tauri
cargo tauri --version  # 应显示 v2.x

# 项目依赖
cd camera-ftp-companion
bun install
cargo check
```

---

## IDE配置

### VS Code推荐设置

安装以下扩展：
- **Rust Analyzer** - Rust语言支持
- **ESLint** - JavaScript/TypeScript检查
- **Prettier** - 代码格式化
- **Tailwind CSS IntelliSense** - TailwindCSS智能提示
- **Tauri** - Tauri项目支持

### settings.json

```json
{
  "editor.formatOnSave": true,
  "editor.defaultFormatter": "esbenp.prettier-vscode",
  "[rust]": {
    "editor.defaultFormatter": "rust-lang.rust-analyzer"
  },
  "rust-analyzer.checkOnSave.command": "clippy"
}
```

---

## 故障排除

### 错误：`linker cc not found`

**解决**: 安装C编译器
```bash
# Ubuntu/Debian
sudo apt install build-essential

# Fedora
sudo dnf install gcc

# macOS
xcode-select --install
```

### 错误：`pkg-config not found`

**解决**:
```bash
sudo apt install pkg-config
```

### 错误：`WebKit2GTK not found`

**解决**:
```bash
sudo apt install libwebkit2gtk-4.0-dev
```

### WSL中无法访问Windows Rust

**解决**: 检查Windows用户名路径
```bash
ls /mnt/c/Users/
# 确认你的用户名，然后设置正确的路径
export WINDOWS_CARGO="/mnt/c/Users/YOUR_USERNAME/.cargo/bin/cargo.exe"
```

---

## 下一步

环境配置完成后：

1. 阅读[架构文档](ARCHITECTURE.md)了解项目结构
2. 运行 `cargo tauri dev` 启动开发服务器
3. 开始开发！