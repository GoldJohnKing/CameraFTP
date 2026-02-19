# 📸 Camera FTP Companion（图传伴侣）

一款跨平台的相机FTP伴侣应用，让相机照片直接传输到电脑或手机。

[English](README.en.md) | [中文](README.md)

![版本](https://img.shields.io/badge/version-0.1.0-blue)
![平台](https://img.shields.io/badge/platform-Windows%20%7C%20Android-brightgreen)
![技术栈](https://img.shields.io/badge/tech-Tauri%20%2B%20React%20%2B%20Rust-orange)

---

## ✨ 功能特性

- 🚀 **一键启动** - 自动检测IP，智能选择端口
- 📡 **FTP服务器** - 内置FTP服务，支持相机直接上传
- 📊 **实时统计** - 显示已接收照片数、数据量
- 📋 **连接信息** - 一键复制相机配置
- 🖥️ **系统托盘** - 最小化后台运行
- 💾 **配置持久化** - 自动保存用户设置

---

## 🚀 快速开始

### Windows用户

1. **下载**：[Releases页面](releases)下载 `camera-ftp-companion.exe`
2. **运行**：双击EXE文件启动
3. **启动服务器**：点击"启动服务器"按钮
4. **配置相机**：在相机FTP设置中输入显示的连接信息

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
- **Tauri CLI** v2.x

### 安装依赖

```bash
bun install --no-cache
```

### 开发模式

```bash
# 热重载开发
cargo tauri dev
```

### 生产构建

```bash
# Windows完整构建
./build-full.sh

# 输出: release/camera-ftp-companion.exe
```

---

## 📁 项目结构

```
camera-ftp-companion/
├── src/                    # React前端源码
│   └── components/         # UI组件
├── src-tauri/              # Rust后端源码
│   ├── src/
│   │   ├── ftp/           # FTP服务器实现
│   │   └── ...
│   └── target/            # 编译输出
├── release/               # 发布包
└── ARCHITECTURE.md        # 详细架构文档
```

---

## 🏗️ 技术架构

```
React + TypeScript + TailwindCSS
            ↓
      Tauri IPC
            ↓
   Rust + Tokio (FTP Server)
```

- **前端**: React + TypeScript + TailwindCSS
- **框架**: Tauri v2（跨平台）
- **后端**: Rust + Tokio异步运行时
- **构建**: Vite + Cargo

---

## 📖 文档

- [架构文档](ARCHITECTURE.md) - 详细技术架构
- [开发指南](ARCHITECTURE.md#开发指南) - 开发环境配置
- [API参考](ARCHITECTURE.md#技术参考) - 相关技术文档

---

## 🐛 常见问题

**Q: 端口被占用？**  
A: 应用会自动切换到下一个可用端口，无需手动配置。

**Q: 无法获取IP？**  
A: 确保电脑已连接到WiFi或以太网。

**Q: 相机连接失败？**  
A: 检查防火墙设置，确保电脑和相机在同一网络。

更多问题见[架构文档](ARCHITECTURE.md#常见问题)。

---

## 🤝 贡献

欢迎提交Issue和PR！

1. Fork本仓库
2. 创建特性分支 (`git checkout -b feature/AmazingFeature`)
3. 提交更改 (`git commit -m 'feat: 添加某某功能'`)
4. 推送分支 (`git push origin feature/AmazingFeature`)
5. 创建Pull Request

---

## 📄 许可证

[MIT](LICENSE) © 2025 Camera FTP Companion

---

**让摄影工作流更简单！** 📷