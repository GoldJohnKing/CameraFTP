# Android 高性能图片查看器设计文档

**日期**: 2026-03-20
**版本**: 5.1

---

## 1. 目标

为 Android 平台设计内置高性能图片查看器：
- 支持 ≥100MP 图片流畅预览
- ≥60FPS 流畅度
- 高质量无极缩放（1x ~ 5x 任意比例）
- 作为 `openImageWithChooser` 外部应用方案的补充选项，用户可通过配置切换

### 1.1 支持的图片格式

| 格式 | 状态 | 说明 |
|------|------|------|
| JPEG | ✅ | 相机照片，主要目标格式 |

---

## 2. 技术方案

### 2.1 核心思路

使用 Android 原生 `BitmapRegionDecoder` + `SubsamplingScaleImageView` 实现大图浏览。

`SubsamplingScaleImageView` 内部使用 `BitmapRegionDecoder` 实现 tile-based 渲染：

```
图片加载
    │
    ▼
SubsamplingScaleImageView
    ├─ 生成低分辨率基础层（全图预览）
    ├─ 根据当前缩放级别 + 视口位置
    ├─ BitmapRegionDecoder 解码可见区域的高清 tile
    └─ 叠加渲染，自动管理 tile 生命周期
    │
    ▼
用户缩放/平移
    ├─ 缩放时：动态切换 tile 分辨率
    ├─ 平移时：加载新视口 tile，卸载不可见 tile
    └─ 内存恒定，不随图片尺寸增长
```

**关键特性（库内置）：**
- 捏合缩放 + 双击缩放
- 平移 + 惯性滑动
- EXIF Orientation 自动旋转
- 低分辨率预览 + 高清 tile 叠加
- 内存自动管理（LRU tile 缓存）
- 动画过渡（缩放、平移到指定区域）

---

## 3. 架构设计

```
┌─────────────────────────────────────────────────────────────────┐
│  Frontend (React + TypeScript)                                  │
│  - GalleryCard（点击图片 → 根据配置选择打开方式）                  │
│  - ConfigCard（新增：图片打开方式切换）                            │
└───────────────────────────┬─────────────────────────────────────┘
                            │ JS Bridge (window.ImageViewerAndroid)
┌───────────────────────────┴─────────────────────────────────────┐
│  Android Native (Kotlin)                                        │
│  ├─ ImageViewerActivity                                         │
│  │   ├─ ViewPager2（滑动翻页）                                   │
│  │   ├─ SubsamplingScaleImageView（渲染 + 手势）                 │
│  │   ├─ ExifOverlayView（EXIF 信息展示）                         │
│  │   └─ NavigationIndicator（图片位置指示器）                     │
│  └─ ImageViewerBridge（JS Bridge，状态回调）                     │
└─────────────────────────────────────────────────────────────────┘
                            │ Tauri IPC (仅 EXIF)
┌───────────────────────────┴─────────────────────────────────────┐
│  Backend (Rust) — 仅用于 EXIF 解析                               │
│  └─ commands/exif.rs（已有，直接复用）                            │
└─────────────────────────────────────────────────────────────────┘
```

---

## 4. 核心接口

### 4.1 Kotlin 数据结构

```kotlin
data class ViewerState(
    val isOpen: Boolean,
    val currentIndex: Int,
    val totalCount: Int,
    val currentUri: String,
    val scale: Float,
    val isLoading: Boolean,
)
```

### 4.2 JS Bridge 接口

**新增 `ImageViewerBridge`（`window.ImageViewerAndroid`）：**

```typescript
interface ImageViewerAndroid {
  openViewer(uri: string, allUrisJson: string): boolean;
  closeViewer(): boolean;
  onStateChanged(stateJson: string): void;
}
```

### 4.3 配置

```rust
pub struct ImageViewerConfig {
    pub open_method: ImageOpenMethod,  // BuiltInViewer / ExternalApp
    pub max_zoom: f32,                 // 默认 5.0
    pub show_exif: bool,               // 默认 true
}
```

---

## 5. 数据流

### 5.1 打开图片

```
用户点击 GalleryCard 中的图片
    │
    ▼
检查配置 open_method
    ├─ ExternalApp → window.GalleryAndroid.open_external_gallery()（现有逻辑）
    └─ BuiltInViewer → ↓
    │
    ▼
window.ImageViewerAndroid.openViewer(uri, allUrisJson)
    │
    ▼
ImageViewerActivity 启动
    ├─ 接收 URI 列表 + 目标索引
    ├─ SubsamplingScaleImageView 加载 JPEG 图片
    │   ├─ BitmapRegionDecoder 解码
    │   ├─ 生成低分辨率基础层
    │   └─ 按需加载高清 tile
    ├─ 通过 Tauri IPC 获取 EXIF 信息
    └─ 渲染 EXIF 浮层
```

### 5.2 手势处理

手势完全由 `SubsamplingScaleImageView` 内置处理：

| 手势 | 行为 | 实现 |
|------|------|------|
| 双指捏合 | 缩放 1x ~ max_zoom | 库内置 |
| 单指拖动 | 平移（缩放 > 1 时） | 库内置 |
| 双击 | 1x ↔ 2x 切换 | 库内置 |
| 滑动翻页 | 左右滑动切换图片 | ViewPager2 |
| 返回键 | 关闭查看器 | Activity `onBackPressed` |

### 5.3 图片导航

使用 `ViewPager2` 实现滑动翻页：

```
ViewPager2
    ├─ 每页一个 SubsamplingScaleImageView
    ├─ 预加载前后各 1 页（共 3 页内存）
    ├─ 页面切换时：
    │   ├─ 更新 currentIndex
    │   ├─ 加载新图片 EXIF
    │   └─ emit viewer-state-update
    └─ 惯性滑动 + 页面吸附
```

---

## 6. 实现细节

### 6.1 ImageViewerActivity

```kotlin
class ImageViewerActivity : AppCompatActivity() {

    companion object {
        private const val TAG = "ImageViewerActivity"
        const val EXTRA_URIS = "uris"
        const val EXTRA_TARGET_INDEX = "target_index"

        fun start(context: Context, uris: List<String>, targetIndex: Int) {
            val intent = Intent(context, ImageViewerActivity::class.java).apply {
                putExtra(EXTRA_URIS, JSONArray(uris).toString())
                putExtra(EXTRA_TARGET_INDEX, targetIndex)
            }
            context.startActivity(intent)
        }
    }

    private lateinit var viewPager: ViewPager2
    private lateinit var exifOverlay: ExifOverlayView
    private lateinit var navIndicator: TextView
    private var uris: List<String> = emptyList()
    private var currentIndex: Int = 0

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        hideSystemBars()
        setContentView(R.layout.activity_image_viewer)

        uris = parseUrisFromIntent()
        currentIndex = intent.getIntExtra(EXTRA_TARGET_INDEX, 0)

        viewPager = findViewById(R.id.view_pager)
        viewPager.adapter = ImageViewerAdapter(uris)
        viewPager.setCurrentItem(currentIndex, false)
        viewPager.registerOnPageChangeCallback(object : OnPageChangeCallback() {
            override fun onPageSelected(position: Int) {
                currentIndex = position
                updateNavIndicator()
                loadExifForImage(uris[position])
                notifyStateChange()
            }
        })
    }

    private fun hideSystemBars() {
        WindowCompat.setDecorFitsSystemWindows(window, false)
        WindowInsetsControllerCompat(window, window.decorView).apply {
            hide(WindowInsetsCompat.Type.systemBars())
            systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        }
    }
}
```

### 6.2 ImageViewerAdapter（ViewPager2）

```kotlin
class ImageViewerAdapter(
    private val uris: List<String>
) : RecyclerView.Adapter<ImageViewerAdapter.ViewHolder>() {

    class ViewHolder(val imageView: SubsamplingScaleImageView) : RecyclerView.ViewHolder(imageView)

    override fun onCreateViewHolder(parent: ViewGroup, viewType: Int): ViewHolder {
        val imageView = SubsamplingScaleImageView(parent.context).apply {
            layoutParams = ViewGroup.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.MATCH_PARENT
            )
            setMinimumScaleType(SubsamplingScaleImageView.SCALE_TYPE_CENTER_INSIDE)
            setMaxScale(5f)
            setDoubleTapZoomScale(2f)
            setOrientation(SubsamplingScaleImageView.ORIENTATION_USE_EXIF)
            setPanLimit(SubsamplingScaleImageView.PAN_LIMIT_INSIDE)
        }
        return ViewHolder(imageView)
    }

    override fun onBindViewHolder(holder: ViewHolder, position: Int) {
        val uri = Uri.parse(uris[position])
        holder.imageView.setImage(ImageSource.uri(uri))
    }

    override fun getItemCount() = uris.size
}
```

### 6.3 前端集成

修改 `GalleryCard.tsx` 中的图片点击逻辑，根据配置选择打开方式：

```typescript
// 现有代码（约第 355 行附近）：
} else if (window.PermissionAndroid?.openImageWithChooser) {
  window.PermissionAndroid.openImageWithChooser(image.path);
}

// 修改为：
} else if (config.openMethod === 'BuiltInViewer' && window.ImageViewerAndroid?.openViewer) {
  const allUris = images.map(img => img.path);
  window.ImageViewerAndroid.openViewer(image.path, JSON.stringify(allUris));
} else if (window.GalleryAndroid?.open_external_gallery) {
  const allUris = images.map(img => img.path);
  window.GalleryAndroid.open_external_gallery(image.path, JSON.stringify(allUris));
}
```

在 `ConfigCard.tsx` 中新增图片打开方式切换：

```tsx
<ToggleSwitch
  label="内置图片查看器"
  description="使用应用内置查看器浏览大图，支持缩放和滑动翻页"
  checked={config.openMethod === 'BuiltInViewer'}
  onChange={(checked) => updateConfig({ openMethod: checked ? 'BuiltInViewer' : 'ExternalApp' })}
/>
```

---

## 7. 测试策略

### 7.1 单元测试（Kotlin / Robolectric）

| 模块 | 测试内容 |
|------|----------|
| `ImageViewerActivity` | Intent 解析、状态管理、生命周期 |
| `ImageViewerAdapter` | ViewPager 数据绑定、URI 处理 |

### 7.2 集成测试

| 场景 | 验证内容 |
|------|----------|
| 5MP JPEG | 快速加载，流畅缩放 |
| 24MP JPEG | 内存 < 80MB，流畅缩放 |
| 100MP JPEG | 内存 < 80MB（tile 模式），流畅缩放 |
| 快速缩放 | 60 FPS，无卡顿 |
| 滑动翻页 | 流畅切换，无白屏 |
| 连续浏览 50 张 | 无 OOM，内存稳定 |
| EXIF 旋转 | 自动旋转，方向正确 |
| 配置切换 | BuiltInViewer ↔ ExternalApp 切换生效 |

---

## 8. 错误处理

| 错误 | 处理方式 |
|------|----------|
| 图片加载失败 | 显示错误占位图 + 重试按钮 |
| URI 无效/无权限 | Toast 提示 + 跳过该图片 |
| EXIF 获取失败 | 隐藏 EXIF 浮层，不影响浏览 |
| 内存不足 | 系统自动回收 tile，无需特殊处理 |

---

## 9. 依赖库

| 用途 | 库 | 版本 | 说明 |
|------|-----|------|------|
| 大图浏览 | `SubsamplingScaleImageView` | 3.10.0 | AOSP 内置，tile-based 渲染 |
| 图片容器 | `ViewPager2` | AndroidX | 滑动翻页 |
| EXIF 解析 | `nom-exif` (Rust) | 2.7+ | 已有，通过 Tauri IPC 复用 |

**Gradle 依赖添加：**

```kotlin
// app/build.gradle.kts
dependencies {
    // 现有依赖...
    implementation("com.davemorrissey.labs:subsampling-scale-image-view:3.10.0")
    implementation("androidx.viewpager2:viewpager2:1.1.0")
}
```

---

## 10. 实现计划

| Phase | 内容 | 时间 |
|-------|------|------|
| 1 | ImageViewerActivity + SubsamplingScaleImageView + ViewPager2 | 3-4 天 |
| 2 | ImageViewerBridge + 前端集成 + EXIF 浮层 | 3-4 天 |
| 3 | 配置项 + 手势优化 + 错误处理 | 2-3 天 |
| 4 | 测试 + 性能调优 + 内存验证 | 2-3 天 |

**总计：2-3 周**

---

## 11. 关键决策

| 决策 | 理由 |
|------|------|
| SubsamplingScaleImageView | AOSP 内置库，tile-based 渲染，内存恒定 |
| ViewPager2 实现翻页 | 官方推荐，内置惯性滑动 + 页面吸附 |
| 复用现有 EXIF 命令 | Rust 端已有完整 EXIF 解析 |
| JS Bridge 通信 | 复用现有架构模式，与 GalleryBridge 一致 |
| 保留外部应用选项 | 内置查看器不覆盖所有格式，用户可自由选择 |
| 仅支持 JPEG | 相机照片为 JPEG，覆盖核心场景，降低复杂度 |

---

## 12. 参考

- [SubsamplingScaleImageView](https://github.com/davemorrissey/subsampling-scale-image-view) - 大图浏览库
- [BitmapRegionDecoder](https://developer.android.com/reference/android/graphics/BitmapRegionDecoder) - Android 大图解码 API
- [ViewPager2](https://developer.android.com/jetpack/androidx/releases/viewpager2) - 滑动翻页组件
- [Tauri v2 Mobile](https://tauri.app/start/) - 移动端集成
