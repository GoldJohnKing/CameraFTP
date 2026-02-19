# 文档索引

本目录包含Camera FTP Companion项目的完整文档。

## 📖 必读文档

| 文档 | 描述 | 目标读者 |
|------|------|----------|
| [README.md](README.md) | 项目简介和快速开始 | 所有用户 |
| [ARCHITECTURE.md](ARCHITECTURE.md) | 详细技术架构和开发指南 | 开发者 |
| [SETUP.md](SETUP.md) | 开发环境设置指南 | 新开发者 |

## 📋 参考文档

| 文档 | 描述 |
|------|------|
| [CHEATSHEET.md](CHEATSHEET.md) | 快速参考卡片（常用命令） |
| [CHANGELOG.md](CHANGELOG.md) | 版本更新日志 |

## 🔧 构建脚本

| 脚本 | 用途 |
|------|------|
| `build-full.sh` | 完整构建（前端+后端+打包） |
| `build-windows.sh` | 仅构建Windows后端 |
| `build-windows.bat` | Windows批处理构建脚本 |
| `copy-to-desktop.sh` | 复制EXE到Windows桌面 |

## 📁 目录结构

```
camera-ftp-companion/
├── 📄 文档文件
│   ├── README.md            ← 从这里开始
│   ├── ARCHITECTURE.md      ← 详细技术文档
│   ├── SETUP.md             ← 环境设置
│   ├── CHEATSHEET.md        ← 快速参考
│   └── CHANGELOG.md         ← 更新日志
│
├── 📄 配置文件
│   ├── package.json
│   ├── tsconfig.json
│   └── ...
│
├── 📁 src/                  ← React前端
├── 📁 src-tauri/            ← Rust后端
├── 📁 release/              ← 发布包
└── 📁 docs/                 ← 设计和计划
    └── plans/
```

## 🚀 快速导航

### 我是用户
→ 阅读 [README.md](README.md) 了解如何下载和使用

### 我是开发者
1. → 阅读 [SETUP.md](SETUP.md) 配置开发环境
2. → 阅读 [ARCHITECTURE.md](ARCHITECTURE.md) 了解项目架构
3. → 保存 [CHEATSHEET.md](CHEATSHEET.md) 作为日常参考

### 我要贡献代码
→ 查看 [ARCHITECTURE.md#贡献指南](ARCHITECTURE.md#贡献指南)

---

## 💡 提示

- 所有文档使用Markdown格式
- 建议使用支持Markdown预览的编辑器（如VS Code）
- 文档保持与代码同步更新

---

**最后更新**: 2025-02-19