# 更新日志

所有项目的显著更改都将记录在此文件中。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/)，
并且本项目遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

---

## [0.1.0] - 2025-02-19

### 🎉 初始版本

#### 新增
- 基础FTP服务器实现（被动模式）
- React前端界面（3个核心卡片组件）
- Windows EXE完整打包
- 自动IP地址检测
- 智能端口选择（冲突自动切换）
- 实时传输统计
- 连接信息一键复制
- 系统托盘支持（Windows）
- 配置持久化

#### 技术栈
- Tauri v2 框架
- React + TypeScript + TailwindCSS
- Rust + Tokio
- Vite构建工具

#### 支持平台
- Windows 10/11

---

## 待办事项 / 路线图

### [0.2.0] - 计划
- [ ] Android APK支持
- [ ] 前台服务保活
- [ ] 缩略图预览
- [ ] 自动打开照片

### [0.3.0] - 计划
- [ ] 多相机连接支持
- [ ] 传输速度显示
- [ ] 文件类型过滤
- [ ] 深色模式

### [1.0.0] - 计划
- [ ] 安装程序（MSI）
- [ ] 自动更新
- [ ] 多语言支持
- [ ] 云存储同步

---

## 版本说明

- **MAJOR**: 不兼容的API更改
- **MINOR**: 向后兼容的功能添加
- **PATCH**: 向后兼容的问题修复

---

[0.1.0]: https://github.com/yourusername/camera-ftp-companion/releases/tag/v0.1.0