# 📸 Camera FTP Companion（图传伴侣）

一款跨平台的相机FTP伴侣应用，让相机照片直接传输到电脑或手机。

![版本](https://img.shields.io/badge/version-0.1.0-blue)
![平台](https://img.shields.io/badge/platform-Windows%20%7C%20Android-brightgreen)
![技术栈](https://img.shields.io/badge/tech-Tauri%20%2B%20React%20%2B%20Rust-orange)

---

## ✨ 功能特性

- 🚀 **一键启动** - 自动检测IP，智能选择端口
- 📡 **FTP服务器** - 内置FTP服务，支持相机直接上传
- 📊 **实时统计** - 显示已接收照片数、数据量
- 📋 **连接信息** - 一键复制相机配置
- 🖥️ **系统托盘** - Windows最小化后台运行
- 🔐 **存储权限** - Android所有文件访问权限管理
- 💾 **配置持久化** - 自动保存用户设置

---

## 🚀 快速开始

### Windows用户

1. **下载**：[Releases页面](../../releases)下载 `camera-ftp-companion.exe`
2. **运行**：双击EXE文件启动
3. **启动服务器**：点击"启动服务器"按钮
4. **配置相机**：在相机FTP设置中输入显示的连接信息

### Android用户

1. **下载**：安装APK文件
2. **授予权限**：允许所有文件访问权限
3. **选择存储路径**：设置照片保存位置
4. **启动服务器**：点击启动按钮

### 相机设置示例

```
协议: FTP (被动模式)
服务器: 192.168.1.100
端口: 21
用户名: anonymous
密码: (任意)
```

---

## 🛠️ 开发指南

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
# Windows构建
./build-full.sh

# Android构建
./build-android.sh release
```

---

## 🏗️ 技术架构

```
React + TypeScript + TailwindCSS
            ↓
      Tauri IPC
            ↓
    Rust + libunftp (FTP Server)
```

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
