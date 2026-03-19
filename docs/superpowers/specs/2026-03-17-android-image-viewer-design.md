# Android 高性能图片查看器设计文档

**日期**: 2026-03-19
**版本**: 4.0

---

## 1. 目标

为 Android 平台设计高性能图片查看器：
- 支持 ≥100MP 图片预览
- ≥60FPS 流畅度
- 高质量无极缩放（1x ~ 5x 任意比例）

### 1.1 支持的图片格式

| 格式 | 状态 | 说明 |
|------|------|------|
| JPEG | ✅ | 相机照片，主要目标格式 |
| HEIF/HEIC | ❌ | 暂不支持 |

---

## 2. 技术方案

### 2.1 动态渲染模式

根据图片尺寸自动选择渲染策略：

| 模式 | 适用范围 | 策略 | 内存示例 |
|------|----------|------|----------|
| 直接模式 | < 10MP | 原图上传 GPU，硬件缩放 | 5MP: ~30MB |
| 轻量模式 | 10-50MP | 保留原图，CPU 按需重采样 | 24MP: ~84MB |
| 完整模式 | > 50MP | 原图 + 缩略图，高质量无极缩放 | 100MP: ~332MB |

### 2.2 渲染流程

```
图片加载
    │
    ├── < 10MP ──────────────────────────────────────────┐
    │   直接模式：原图 → GPU 纹理 → 硬件缩放              │
    │                                                    │
    ├── 10-50MP ─────────────────────────────────────────┤
    │   轻量模式：原图 → 视口裁剪 → Lanczos3 → GPU       │
    │                                                    │
    └── > 50MP ──────────────────────────────────────────┤
        完整模式：                                       │
          scale ≤ 0.5: 缩略图 → 视口裁剪 → Lanczos3 → GPU
          scale > 0.5: 原图 → 视口裁剪 → Lanczos3 → GPU
```

---

## 3. 架构设计

```
┌─────────────────────────────────────────────────────────────────┐
│  Frontend (React + TypeScript)                                  │
│  - ImageViewerOverlay（EXIF、导航指示器、操作按钮）               │
└───────────────────────────┬─────────────────────────────────────┘
                            │ Tauri IPC
┌───────────────────────────┴─────────────────────────────────────┐
│  Bridge Layer                                                   │
│  - Commands: open_image_viewer, close_image_viewer              │
│  - Events: viewer-state-update                                  │
└───────────────────────────┬─────────────────────────────────────┘
                            │
┌───────────────────────────┴─────────────────────────────────────┐
│  Backend (Rust)                                                 │
│  ├─ ImageLoader（图像解码、模式选择、缩略图生成）                  │
│  ├─ ViewportRenderer（视口裁剪、动态重采样）                      │
│  ├─ GpuRenderer（wgpu GPU 渲染）                                │
│  ├─ GestureHandler（手势识别）                                   │
│  └─ AndroidBridge（JNI 桥接）                                    │
└─────────────────────────────────────────────────────────────────┘
```

---

## 4. 核心接口

### 4.1 数据结构

```rust
/// 渲染模式
pub enum RenderMode {
    Direct,      // < 10MP
    Lightweight, // 10-50MP
    Full,        // > 50MP
}

impl RenderMode {
    pub fn from_dimensions(width: u32, height: u32) -> Self {
        let mp = (width as u64 * height as u64) as f32 / 1_000_000.0;
        if mp < 10.0 { Direct }
        else if mp < 50.0 { Lightweight }
        else { Full }
    }
}

/// 加载后的图像
pub struct LoadedImage {
    pub mode: RenderMode,
    pub original: Arc<RgbImage>,
    pub thumbnail: Option<Arc<RgbImage>>,  // 仅完整模式
    pub exif: Option<ExifData>,
    pub orientation: ExifOrientation,
}

/// 视口状态
pub struct Viewport {
    pub x: f32,      // 视口位置 X
    pub y: f32,      // 视口位置 Y
    pub scale: f32,  // 缩放级别 (1.0 = 100%)
    pub width: u32,  // 屏幕宽度
    pub height: u32, // 屏幕高度
}

/// 渲染输出
pub enum RenderOutput {
    FullTexture { data: Vec<u8>, width: u32, height: u32 },
    ViewportTexture { data: Vec<u8>, width: u32, height: u32 },
}

/// 手势动作
pub enum GestureAction {
    Pan { dx: f32, dy: f32 },
    Zoom { scale: f32, center: (f32, f32) },
    SwitchImage { direction: i8 },
    ToggleZoom,
}
```

### 4.2 核心模块

| 模块 | 文件 | 职责 |
|------|------|------|
| `image_loader` | `loader.rs` | 解码、模式选择、缩略图生成 |
| `viewport_renderer` | `viewport.rs` | 视口计算、裁剪、重采样 |
| `gpu_renderer` | `gpu.rs` | GPU 纹理管理、渲染 |
| `gesture_handler` | `gesture.rs` | 手势识别、视口计算 |
| `android_surface` | `android.rs` | SurfaceView 生命周期 |
| `viewer_bridge` | `bridge.rs` | Tauri 命令、事件 |

---

## 5. 数据流

### 5.1 打开图片

```
用户点击图片
    │
    ▼
invoke('open_image_viewer', { path, index, total })
    │
    ▼
ImageLoader::load(path)
    ├─ 解码图片
    ├─ 选择渲染模式
    └─ 生成缩略图（如需要）
    │
    ▼
GpuRenderer 渲染初始视口
    │
    ▼
emit('viewer-state-update', state)
```

### 5.2 手势处理

```
用户触摸屏幕
    │
    ▼
GestureDetector 识别手势
    │
    ├─ 双指捏合 → Zoom
    ├─ 单指拖动 (scale > 1) → Pan
    ├─ 单指滑动 (scale = 1) → SwitchImage
    └─ 双击 → ToggleZoom
    │
    ▼
ViewportRenderer::render(viewport)
    │
    ├─ 选择源图像（原图/缩略图）
    ├─ 计算裁剪区域
    └─ Lanczos3 重采样
    │
    ▼
GpuRenderer::render(texture)
```

### 5.3 手势交互规则

| 手势 | 条件 | 行为 |
|------|------|------|
| 双指捏合 | 任意 | 缩放 1x ~ 5x |
| 单指拖动 | scale > 1 | 平移 |
| 单指滑动 | scale = 1 | 切换上/下一张 |
| 双击 | 任意 | 1x ↔ 2x |
| 返回键 | 任意 | 关闭查看器 |

---

## 6. 配置

```rust
pub struct ImageViewerConfig {
    pub open_method: ImageOpenMethod,  // BuiltInViewer / ExternalApp
    pub max_zoom: f32,                 // 默认 5.0
    pub show_exif: bool,               // 默认 true
}
```

---

## 7. 测试策略

### 7.1 单元测试

| 模块 | 测试内容 |
|------|----------|
| `image_loader` | 模式选择、缩略图生成 |
| `viewport_renderer` | 裁剪区域计算、重采样质量 |
| `gesture_handler` | 手势识别、边界处理 |

### 7.2 集成测试

| 场景 | 验证内容 |
|------|----------|
| 5MP 图片 | 直接模式，内存 ~30MB |
| 24MP 图片 | 轻量模式，内存 ~84MB |
| 100MP 图片 | 完整模式，内存 ~332MB |
| 快速缩放 | 60 FPS |
| 无极缩放 | 任意比例保持清晰 |
| 内存压力 | 连续浏览 50 张，无 OOM |

---

## 8. 错误处理

| 错误 | 处理方式 |
|------|----------|
| 加载失败 | 错误提示 + 重试按钮 |
| GPU 失败 | 降级到 WebView |
| 内存不足 | 提示用户 |
| 格式不支持 | 提示用外部应用 |

---

## 9. 依赖库

| 用途 | 库 | 说明 |
|------|-----|------|
| 图像解码 | `image` 0.25+ | 内部使用 zune-jpeg |
| 图像缩放 | `fast_image_resize` 5.0+ | SIMD 加速，Lanczos3 |
| GPU 渲染 | `wgpu` 0.19+ | 跨平台 GPU API |
| EXIF 解析 | `nom-exif` 2.7+ | 已在项目中使用 |

---

## 10. 实现计划

| Phase | 内容 | 时间 |
|-------|------|------|
| 1 | 基础架构：ImageLoader、ViewportRenderer | Week 1 |
| 2 | Android 原生层：Activity、SurfaceView | Week 2 |
| 3 | GPU 渲染：wgpu 集成 | Week 3 |
| 4 | 手势处理：识别、视口联动 | Week 4 |
| 5 | Frontend：UI 覆盖层 | Week 5 |
| 6 | 配置与测试 | Week 6 |

---

## 11. 关键决策

| 决策 | 理由 |
|------|------|
| 使用 `image` crate | v0.25 内部使用 zune-jpeg，生态成熟 |
| 动态渲染模式 | 根据图片尺寸优化内存占用 |
| 保留原图 + Lanczos3 | 实现高质量无极缩放 |
| 纯 Rust 实现 | 简化 Android 跨平台编译 |

---

## 12. 参考

- [image crate](https://docs.rs/image/latest/image/)
- [fast_image_resize](https://github.com/Cykooz/fast_image_resize)
- [wgpu](https://wgpu.rs/)
- [Tauri v2 Mobile](https://tauri.app/start/)
