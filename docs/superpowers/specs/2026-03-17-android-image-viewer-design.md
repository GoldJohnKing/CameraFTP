# Android 高性能图片查看器设计文档

**日期**: 2026-03-17  
**版本**: 2.0  
**状态**: 设计完成，已通过审查

---

## 1. 目标

为 Android 平台设计一个高性能图片查看器，支持 ≥100MP 图片预览，保证 ≥60FPS 流畅度和 5x 高清缩放。

---

## 2. 架构设计

### 2.1 整体架构

采用"纯原生 GPU 渲染"方案：
- **Frontend**: 仅负责 UI 控件（EXIF 显示、按钮、导航指示器）
- **Backend**: 使用 wgpu 直接渲染到 Android SurfaceView
- **手势处理**: 完全在原生层实现，避免 WebView 性能瓶颈

```
┌─────────────────────────────────────────────────────────────────┐
│                        表现层 (Frontend)                         │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  React + TypeScript                                     │   │
│  │  - ImageViewerOverlay（EXIF 信息、导航指示器、操作按钮）     │   │
│  │  - 状态同步（通过 Tauri IPC 获取当前图片索引、缩放级别）       │   │
│  └─────────────────────────────────────────────────────────┘   │
│                          ↕ Tauri IPC                           │
├─────────────────────────────────────────────────────────────────┤
│                        调度层 (Bridge)                          │
│  ├─ Tauri Commands: open_image_viewer, close_image_viewer      │
│  └─ Tauri Events: viewer-state-update (图片切换、缩放变化)       │
├─────────────────────────────────────────────────────────────────┤
│                        核心层 (Backend)                         │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  Rust Engine                                            │   │
│  │  ├─ ImageLoader（图像解码、金字塔构建）                   │   │
│  │  ├─ TileManager（动态分片、LRU 缓存）                    │   │
│  │  ├─ GpuRenderer（wgpu GPU 渲染）                        │   │
│  │  ├─ GestureHandler（原生手势识别）                       │   │
│  │  └─ AndroidBridge（JNI 桥接、SurfaceView 管理）          │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

**架构决策理由**:
- OpenSeadragon 与 wgpu GPU 渲染是互斥方案，无法协同工作
- 纯原生 GPU 方案可获得最佳性能和 100MP+ 支持
- Frontend 仅作为 UI 覆盖层，不参与渲染流程

---

## 3. 模块边界与接口

### 3.1 Rust 后端模块

| 模块 | 文件路径 | 职责 | 对外接口 |
|------|----------|------|----------|
| `image_loader` | `src-tauri/src/image_viewer/loader.rs` | 图像解码、金字塔构建 | `ImageLoader::load(path) -> Result<ImagePyramid, ImageError>` |
| `tile_manager` | `src-tauri/src/image_viewer/tile.rs` | 分片生成、LRU 缓存 | `TileManager::get_tile(key) -> Result<TileData, TileError>` |
| `gpu_renderer` | `src-tauri/src/image_viewer/gpu.rs` | GPU 纹理管理、渲染 | `GpuRenderer::render(viewport, tiles) -> Result<RenderStats, RenderError>` |
| `gesture_handler` | `src-tauri/src/image_viewer/gesture.rs` | 手势识别、视口计算 | `on_touch_event(event) -> GestureAction` |
| `android_surface` | `src-tauri/src/image_viewer/android.rs` | SurfaceView 生命周期 | `attach(window) -> Result<ANativeWindow, SurfaceError>` |
| `viewer_bridge` | `src-tauri/src/image_viewer/bridge.rs` | Tauri 命令、事件 | `open_image_viewer(args) / emit_state_update(state)` |

#### 3.1.1 完整接口定义

```rust
/// 图像金字塔结构
pub struct ImagePyramid {
    pub levels: Vec<PyramidLevel>,
    pub width: u32,
    pub height: u32,
    pub orientation: ExifOrientation,
}

pub struct PyramidLevel {
    pub scale: f32,              // 缩放比例 (1.0, 0.5, 0.25...)
    pub width: u32,
    pub height: u32,
    pub data: Arc<DynamicImage>, // 该层级的完整图像数据
}

/// 分片键
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TileKey {
    pub level: u8,  // 金字塔层级
    pub x: u32,     // 分片 X 坐标
    pub y: u32,     // 分片 Y 坐标
}

/// 分片数据
pub struct TileData {
    pub key: TileKey,
    pub data: Vec<u8>,         // JPEG 编码数据
    pub width: u32,
    pub height: u32,
}

/// 视口状态
pub struct Viewport {
    pub x: f32,      // 视口左上角 X（图像坐标）
    pub y: f32,      // 视口左上角 Y（图像坐标）
    pub scale: f32,  // 缩放级别 (1.0 = 100%)
    pub width: u32,  // 视口宽度（屏幕像素）
    pub height: u32, // 视口高度（屏幕像素）
}

/// 渲染结果统计
pub struct RenderStats {
    pub draw_calls: u32,
    pub tiles_rendered: u32,
    pub frame_time_ms: f32,
}

/// 手势动作
#[derive(Debug, Clone)]
pub enum GestureAction {
    None,
    Pan { dx: f32, dy: f32 },           // 平移
    Zoom { scale: f32, center: (f32, f32) }, // 缩放
    SwitchImage { direction: i8 },      // -1 = 上一张, 1 = 下一张
    ToggleZoom,                         // 双击切换缩放
}

/// 查看器状态（用于 Frontend 同步）
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ViewerState {
    pub current_index: usize,
    pub total_count: usize,
    pub current_path: String,
    pub scale: f32,
    pub is_exif_visible: bool,
    pub has_next: bool,
    pub has_prev: bool,
}
```

#### 3.1.2 错误类型定义

```rust
#[derive(Debug, thiserror::Error)]
pub enum ImageError {
    #[error("Failed to decode image: {0}")]
    DecodeError(String),
    #[error("Unsupported format")]
    UnsupportedFormat,
    #[error("File not found: {0}")]
    NotFound(String),
}

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("GPU context lost")]
    ContextLost,
    #[error("Out of memory")]
    OutOfMemory,
    #[error("Invalid viewport")]
    InvalidViewport,
    #[error("Surface not available")]
    SurfaceNotAvailable,
}

#[derive(Debug, thiserror::Error)]
pub enum SurfaceError {
    #[error("Failed to attach to window")]
    AttachFailed,
    #[error("Surface disconnected")]
    Disconnected,
}
```

### 3.2 Android 原生层模块

| 模块 | 文件路径 | 职责 | 关键方法 |
|------|----------|------|----------|
| `ImageViewerActivity` | `gen/android/.../viewer/ImageViewerActivity.kt` | 管理生命周期、启动 Surface | `onCreate()`, `onDestroy()` |
| `NativeSurfaceView` | `gen/android/.../viewer/NativeSurfaceView.kt` | 创建 SurfaceView、处理触摸事件 | `surfaceCreated()`, `onTouchEvent()` |
| `GestureDetector` | `gen/android/.../viewer/GestureDetector.kt` | 识别手势、转发到 Rust | `onScale()`, `onScroll()`, `onDoubleTap()` |

```kotlin
// NativeSurfaceView.kt
class NativeSurfaceView(context: Context) : SurfaceView(context), SurfaceHolder.Callback {
    
    init {
        holder.addCallback(this)
        setZOrderOnTop(true)  // 确保 SurfaceView 在 WebView 之上
    }
    
    override fun surfaceCreated(holder: SurfaceHolder) {
        // 获取 ANativeWindow 并传递给 Rust
        val window = holder.surface
        RustBridge.attachSurface(window)
    }
    
    override fun onTouchEvent(event: MotionEvent): Boolean {
        // 转发触摸事件到 Rust
        RustBridge.onTouchEvent(event.action, event.x, event.y, event.pointerCount)
        return true
    }
}
```

### 3.3 前端模块

| 模块 | 文件路径 | 职责 |
|------|----------|------|
| `ImageViewerOverlay` | `src/components/ImageViewerOverlay.tsx` | UI 覆盖层（EXIF、按钮、导航） |
| `viewerStore` | `src/stores/viewerStore.ts` | 查看器状态管理 |
| `ViewerConfigCard` | `src/components/ViewerConfigCard.tsx` | 配置界面 |

---

## 4. 数据流与交互流程

### 4.1 打开图片流程

```
用户点击图片
    │
    ▼
GalleryCard.tsx 检查配置：内置查看器 or 外部应用
    │
    ▼ (内置查看器)
invoke('open_image_viewer', { path, index, total })
    │
    ▼
viewer_bridge.rs
    ├─ 启动 ImageViewerActivity (通过 JNI)
    ├─ 创建 NativeSurfaceView
    └─ 初始化 Rust 组件：
         ImageLoader::load(path) → ImagePyramid
         GpuRenderer::init(surface)
         TileManager::new(cache_size)
    │
    ▼
GpuRenderer 渲染初始视口（缩放 1x，居中显示）
    │
    ▼
viewer_bridge 发送事件到 Frontend：viewer-state-update
    │
    ▼
ImageViewerOverlay.tsx 显示 UI 覆盖层（EXIF、按钮）
```

### 4.2 手势处理流程（原生层）

```
用户触摸屏幕
    │
    ▼
NativeSurfaceView.onTouchEvent(event)
    │
    ▼
GestureDetector 识别手势类型
    │
    ├─ 双指捏合 ───────────────────────────────┐
    ├─ 单指拖动（缩放状态下）─────────────────────┤
    ├─ 单指滑动（非缩放状态）─────────────────────┤ 所有手势
    ├─ 双击 ────────────────────────────────────┤ 统一转发
    └─ 系统返回键 ──────────────────────────────┘
    │                                          │
    ▼                                          ▼
RustBridge.onTouchEvent()           RustBridge.onBackPressed()
    │                                          │
    ▼                                          ▼
gesture_handler.rs 处理手势            viewer_bridge.rs
    │                                   关闭查看器
    ├─ Pan → 更新 Viewport.x/y
    ├─ Zoom → 更新 Viewport.scale（限制 5x）
    ├─ SwitchImage → 加载新图片
    └─ ToggleZoom → scale 1x ↔ 2x
    │
    ▼
计算新的可见 tiles
    │
    ▼
TileManager::get_tile(key) → 从缓存或实时生成
    │
    ▼
GpuRenderer::render(viewport, tiles) → 60 FPS
    │
    ▼
发送 viewer-state-update 事件到 Frontend（同步状态）
```

### 4.3 手势交互规则

| 手势 | 条件 | 行为 |
|------|------|------|
| 双指捏合 | 任意位置 | 缩放图片（1x ~ 5x） |
| 单指拖动 | scale > 1 | 平移图片（受边界限制） |
| 单指滑动 | scale = 1 | 切换上一张/下一张（带过渡动画） |
| 双击 | 任意位置 | 快速切换：1x ↔ 2x |
| 系统返回键 | 任意状态 | 关闭查看器，返回图库 |

**设计理由**：
- 避免与 Android 系统手势冲突（边缘滑动返回）
- 简化实现，减少误触
- 符合主流相册应用交互习惯

### 4.4 配置读取流程

```
用户打开配置界面
    │
    ▼
ViewerConfigCard.tsx 从 configStore 读取 viewerConfig
    │
    ▼
用户切换"打开方式"选项
    │
    ▼
updateDraft({ viewerConfig: { openMethod: 'external' } })
    │
    ▼
自动保存到 Rust 后端
    │
    ▼
src-tauri/src/config.rs 保存到 config.json
```

---

## 5. 配置设计

### 5.1 数据结构

```rust
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum ImageOpenMethod {
    BuiltInViewer,
    ExternalApp,
}

impl Default for ImageOpenMethod {
    fn default() -> Self {
        ImageOpenMethod::BuiltInViewer
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ImageViewerConfig {
    pub open_method: ImageOpenMethod,
    pub max_zoom: f32,          // 默认 5.0
    pub tile_cache_size: usize, // 默认 50
    pub show_exif: bool,        // 默认 true
}

impl Default for ImageViewerConfig {
    fn default() -> Self {
        Self {
            open_method: ImageOpenMethod::default(),
            max_zoom: 5.0,
            tile_cache_size: 50,
            show_exif: true,
        }
    }
}

// 集成到 AppConfig（使用 Default 而非 Option）
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    // ... 现有字段 ...
    pub viewer_config: ImageViewerConfig,  // 不是 Option
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            // ... 其他字段 ...
            viewer_config: ImageViewerConfig::default(),
        }
    }
}
```

### 5.2 UI 配置项

- **图片打开方式**：单选按钮组
  - 使用内置查看器（默认）
  - 使用外部应用打开
- **最大缩放级别**：滑块（1x - 10x，默认 5x）
- **显示 EXIF 信息**：开关（默认开启）

---

## 6. 测试策略

### 6.1 单元测试覆盖

| 模块 | 测试内容 | 框架 | 测试文件 |
|------|----------|------|----------|
| `image_loader` | 图像解码、金字塔层级计算、EXIF 解析 | `cargo test` | `loader_test.rs` |
| `tile_manager` | LRU 缓存淘汰策略、并发安全 | `cargo test` | `tile_test.rs` |
| `gesture_handler` | 手势识别逻辑、视口计算 | `cargo test` | `gesture_test.rs` |
| `gpu_renderer` | 纹理坐标计算、视口裁剪（使用 mock） | `mockall` + `cargo test` | `gpu_test.rs` |
| `viewer_bridge` | Tauri 命令、错误处理、事件发送 | `cargo test` | `bridge_test.rs` |

### 6.2 关键测试用例

```rust
// image_loader tests
#[test]
fn test_pyramid_level_count() {
    // 100MP (10000x10000) 图片应生成约 5 层金字塔
    // 10000 -> 5000 -> 2500 -> 1250 -> 625
    let pyramid = ImageLoader::load(test_image_100mp()).unwrap();
    assert_eq!(pyramid.levels.len(), 5);
}

#[test]
fn test_orientation_handling() {
    // EXIF 旋转信息正确处理
    let pyramid = ImageLoader::load(test_image_rotated()).unwrap();
    assert_eq!(pyramid.orientation, ExifOrientation::Rotate90);
}

// tile_manager tests
#[test]
fn test_lru_cache_eviction() {
    let mut tm = TileManager::new(3); // 缓存大小为 3
    
    // 插入 4 个 tiles
    tm.cache_tile(TileKey { level: 0, x: 0, y: 0 }, tile_data());
    tm.cache_tile(TileKey { level: 0, x: 1, y: 0 }, tile_data());
    tm.cache_tile(TileKey { level: 0, x: 2, y: 0 }, tile_data());
    tm.cache_tile(TileKey { level: 0, x: 3, y: 0 }, tile_data());
    
    // 最早插入的应被淘汰
    assert!(tm.get_tile(TileKey { level: 0, x: 0, y: 0 }).is_none());
    assert!(tm.get_tile(TileKey { level: 0, x: 1, y: 0 }).is_some());
}

#[test]
fn test_tile_coordinate_calculation() {
    // 验证分片坐标计算正确
    let viewport = Viewport { x: 0.0, y: 0.0, scale: 1.0, width: 1024, height: 1024 };
    let tiles = calculate_visible_tiles(&viewport, 512);
    
    // 视口 1024x1024，tile 大小 512x512，应需要 4 个 tiles
    assert_eq!(tiles.len(), 4);
}

// gesture_handler tests
#[test]
fn test_pan_gesture_at_max_zoom() {
    // 在最大缩放时，平移应受边界限制
    let mut viewport = Viewport { x: 0.0, y: 0.0, scale: 5.0, width: 1080, height: 1920 };
    let action = GestureAction::Pan { dx: -10000.0, dy: 0.0 };
    
    apply_gesture(&mut viewport, action, image_size());
    
    // x 不应小于最小边界
    assert!(viewport.x >= 0.0);
}

#[test]
fn test_switch_image_at_default_zoom() {
    // 在非缩放状态，滑动应触发图片切换
    let viewport = Viewport { x: 0.0, y: 0.0, scale: 1.0, width: 1080, height: 1920 };
    let gesture = detect_gesture(touch_events_swipe_right());
    
    assert!(matches!(gesture, GestureAction::SwitchImage { direction: -1 }));
}
```

### 6.3 集成测试

| 测试场景 | 验证内容 |
|----------|----------|
| 打开 100MP 图片 | 初始化时间 < 500ms，内存使用 < 200MB |
| 快速缩放 | 60 FPS 保持，无卡顿 |
| 快速滑动切换 | 过渡动画流畅，无闪烁 |
| 内存压力测试 | 连续浏览 50 张图片，无 OOM |
| GPU 降级 | 模拟 GPU 失败，正确降级并提示用户 |

---

## 7. 错误处理策略

| 错误类型 | 处理方式 | 用户可见 |
|----------|----------|----------|
| 图像加载失败 | 显示错误提示 + 重试按钮 | "无法加载图片，点击重试" |
| GPU 初始化失败 | 降级到 WebView 渲染 | "使用兼容模式打开" |
| 内存不足 | 清理 LRU 缓存 + 提示 | "内存不足，已清理缓存" |
| 分片生成超时 | 显示低分辨率占位 + 后台重试 | 低质量预览，加载完成后切换 |
| 不支持的格式 | 提示使用外部应用打开 | "该格式不支持，使用外部应用打开" |

**GPU 降级方案**：
- 如果 wgpu 初始化失败，使用 Android WebView 的硬件加速显示图片
- 降级后不支持 100MP+ 图片，提示用户使用外部应用
- 降级状态保存到 session，下次直接使用降级方案

---

## 8. 依赖库

| 用途 | 库名 | 版本 | 说明 |
|------|------|------|------|
| 图像解码 | `zune-image` | 0.5+ | 纯 Rust，高性能解码 |
| 图像缩放 | `fast_image_resize` | 5.0+ | 纯 Rust，SIMD 加速 |
| GPU 渲染 | `wgpu` | 0.19+ | 跨平台 GPU API |
| LRU 缓存 | `lru` | 0.12+ | 标准 LRU 实现 |
| EXIF 解析 | `nom-exif` | 2.7+ | 已在项目中使用 |
| 错误处理 | `thiserror` | 2.0+ | 已在项目中使用 |

**移除的依赖**：
- ~~`libvips`~~：Android 集成过于复杂，使用纯 Rust 方案替代
- ~~`OpenSeadragon`~~：与 GPU 渲染互斥，使用原生手势处理

---

## 9. 实现顺序建议

### Phase 1: 基础架构（Week 1）
- [ ] 创建 `src-tauri/src/image_viewer/` 模块结构
- [ ] 实现 `ImageLoader` + 单元测试
- [ ] 实现 `TileManager` + 单元测试

### Phase 2: Android 原生层（Week 2）
- [ ] 创建 `ImageViewerActivity.kt`
- [ ] 实现 `NativeSurfaceView`
- [ ] JNI 桥接层（attach/detach surface）

### Phase 3: GPU 渲染（Week 3）
- [ ] 集成 `wgpu`
- [ ] 实现 `GpuRenderer`
- [ ] 分片纹理上传和渲染

### Phase 4: 手势与交互（Week 4）
- [ ] 实现 `GestureDetector.kt`
- [ ] 实现 `gesture_handler.rs`
- [ ] 手势 ↔ 视口更新联动

### Phase 5: Frontend 集成（Week 5）
- [ ] `ImageViewerOverlay.tsx`
- [ ] 状态同步（Tauri Events）
- [ ] UI 控件（EXIF、导航指示器）

### Phase 6: 配置与测试（Week 6）
- [ ] `ViewerConfigCard.tsx`
- [ ] 配置持久化
- [ ] 集成测试 + 性能优化

---

## 10. 决策记录

| 日期 | 决策 | 理由 |
|------|------|------|
| 2026-03-17 | 选择完整方案（Native Overlay + Dynamic Tiling） | 最高性能，满足 100MP+ 需求 |
| 2026-03-17 | 使用纯原生 GPU 渲染，移除 OpenSeadragon | 与 GPU 渲染互斥，原生方案性能更好 |
| 2026-03-17 | 手势处理完全在原生层实现 | 避免 WebView 性能瓶颈，获得最佳响应 |
| 2026-03-17 | 实时计算图像金字塔（不缓存） | 简化首次实现，后续可加缓存 |
| 2026-03-17 | 单指滑动切换图片（非缩放状态） | 简单直观，避免与系统手势冲突 |
| 2026-03-17 | 固定 5x 最大缩放 | 平衡性能与实用性 |
| 2026-03-17 | 简单 LRU 缓存（50 tiles） | 实现简单，足够覆盖视口 |
| 2026-03-17 | 使用纯 Rust 图像处理（替代 libvips） | Android 集成更简单，避免 FFI 复杂性 |
| 2026-03-17 | 全局配置 + 记住选择 | 用户意图明确，实现简单 |
| 2026-03-17 | ImageViewerConfig 使用 Default trait（非 Option） | 简化空值处理 |

---

## 11. 参考文档

- [Tauri v2 Mobile](https://tauri.app/start/)
- [wgpu 文档](https://wgpu.rs/)
- [zune-image GitHub](https://github.com/etemesi254/zune-image)
- [fast_image_resize](https://github.com/Cykooz/fast_image_resize)
- [Android SurfaceView](https://developer.android.com/reference/android/view/SurfaceView)
