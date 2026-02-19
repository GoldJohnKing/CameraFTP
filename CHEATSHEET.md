# 快速参考卡片

## 常用命令

### 开发

```bash
# 启动开发服务器（热重载）
cargo tauri dev

# 仅前端开发
bun run dev

# 前端构建
bun run build
```

### 构建

```bash
# 完整Windows构建（WSL）
./build-full.sh

# 仅后端构建
./build-windows.sh

# 复制EXE到Windows桌面
./copy-to-desktop.sh
```

### 代码检查

```bash
# Rust检查
cargo check
cargo clippy

# 格式化代码
cargo fmt

# TypeScript检查
bun run tsc
```

## 项目结构速览

```
📁 src/              # React前端
📁 src-tauri/        # Rust后端
📁 dist/             # 前端构建输出
📁 release/          # 发布包
📄 ARCHITECTURE.md   # 架构文档
📄 README.md         # 项目说明
📄 SETUP.md          # 环境设置
```

## 技术栈

| 层级 | 技术 |
|------|------|
| 前端 | React + TypeScript + TailwindCSS |
| 框架 | Tauri v2 |
| 后端 | Rust + Tokio |
| 构建 | Vite + Cargo |

## 关键文件

| 文件 | 用途 |
|------|------|
| `src-tauri/tauri.conf.json` | Tauri配置 |
| `src-tauri/Cargo.toml` | Rust依赖 |
| `package.json` | Node.js依赖 |
| `vite.config.ts` | Vite配置 |
| `tailwind.config.js` | TailwindCSS配置 |

## 快速修复

| 问题 | 解决 |
|------|------|
| 端口占用 | 自动切换，无需处理 |
| 无IP地址 | 检查网络连接 |
| 编译失败 | 检查Rust/Node版本 |
| 图标错误 | 重新运行build脚本 |

## 版本信息

- **当前版本**: 0.1.0
- **Tauri**: v2.x
- **Rust**: ≥1.75
- **Node**: ≥18
- **Bun**: ≥1.0

---

**提示**: 保存此文件供快速查阅！