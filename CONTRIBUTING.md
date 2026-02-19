# 贡献指南

感谢您对Camera FTP Companion项目的关注！我们欢迎所有形式的贡献。

## 🚀 如何贡献

### 报告问题

如果您发现了bug或有功能建议：

1. 检查 [Issues页面](../../issues) 是否已存在相关问题
2. 如果不存在，创建新的Issue，包含：
   - 清晰的标题
   - 详细的问题描述
   - 复现步骤（对于bug）
   - 期望的行为
   - 截图（如适用）
   - 环境信息（操作系统、版本等）

### 提交代码

1. **Fork** 本仓库
2. **克隆** 到本地：
   ```bash
   git clone https://github.com/YOUR_USERNAME/camera-ftp-companion.git
   ```
3. **创建分支**：
   ```bash
   git checkout -b feature/your-feature-name
   # 或
   git checkout -b fix/issue-description
   ```
4. **提交更改**：
   ```bash
   git commit -m "feat: 添加新功能"
   # 或
   git commit -m "fix: 修复某问题"
   ```
5. **推送** 到您的Fork：
   ```bash
   git push origin feature/your-feature-name
   ```
6. 创建 **Pull Request**

## 📝 提交规范

我们使用 [Conventional Commits](https://www.conventionalcommits.org/) 规范：

### 提交类型

- **feat**: 新功能
- **fix**: Bug修复
- **docs**: 文档更新
- **style**: 代码格式（不影响功能）
- **refactor**: 代码重构
- **perf**: 性能优化
- **test**: 测试相关
- **chore**: 构建过程或辅助工具的变动

### 提交示例

```bash
# 新功能
git commit -m "feat: 添加系统自动启动功能"

# Bug修复
git commit -m "fix: 修复端口冲突时崩溃的问题"

# 文档
git commit -m "docs: 更新README中的安装说明"

# 包含详细说明
git commit -m "feat: 添加FTP传输速度显示

- 在StatsCard中显示实时传输速度
- 使用平滑算法避免数值跳动
- 支持自动切换KB/s和MB/s单位"
```

## 🎨 代码规范

### Rust代码

- 使用 `rustfmt` 格式化代码
- 使用 `clippy` 检查代码
- 遵循 Rust API Guidelines

```bash
# 格式化
cargo fmt

# 检查
cargo clippy -- -D warnings

# 测试
cargo test
```

### TypeScript/React代码

- 使用项目配置的ESLint规则
- 使用Prettier格式化
- 遵循函数组件和Hooks最佳实践

```bash
# 类型检查
bun run tsc

# 构建测试
bun run build
```

## 🏗️ 开发流程

### 1. 环境准备

确保您的开发环境已配置：
- [SETUP.md](SETUP.md) - 详细设置指南

### 2. 创建功能分支

```bash
# 从最新的main分支创建
git checkout main
git pull origin main
git checkout -b feature/my-feature
```

### 3. 开发和测试

- 编写代码
- 添加测试（如适用）
- 确保所有测试通过
- 手动测试功能

### 4. 提交前检查

```bash
# 1. 格式化代码
cargo fmt
bun run format  # 如果有

# 2. 检查代码
cargo clippy
bun run lint    # 如果有

# 3. 构建测试
cargo build --release
bun run build

# 4. 完整构建测试
./build-full.sh
```

### 5. 创建Pull Request

PR描述应包含：
- 变更摘要
- 相关Issue编号（如适用）
- 测试说明
- 截图（如UI变更）

## 📋 PR审查清单

维护者在合并前会检查：

- [ ] 代码符合项目风格
- [ ] 所有测试通过
- [ ] 文档已更新（如需要）
- [ ] 提交信息符合规范
- [ ] 没有不必要的文件更改
- [ ] 功能在目标平台测试通过

## 🎯 开发优先级

当前优先开发的功能：

1. **Android支持** - APK构建和前台服务
2. **性能优化** - 大文件传输优化
3. **用户体验** - 更好的错误提示和引导

查看 [CHANGELOG.md](CHANGELOG.md) 了解完整路线图。

## 💬 沟通渠道

- **GitHub Issues**: Bug报告和功能请求
- **GitHub Discussions**: 一般性讨论和问题
- **Pull Requests**: 代码审查和技术讨论

## 🙏 贡献者

感谢所有为项目做出贡献的人！

<!-- 贡献者列表将由all-contributors自动更新 -->

---

## 📄 许可证

通过贡献代码，您同意您的贡献将在 [MIT许可证](LICENSE) 下发布。

---

**再次感谢您的贡献！** 🎉