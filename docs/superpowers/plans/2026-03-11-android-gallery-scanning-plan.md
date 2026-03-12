# Android图库文件扫描重构 - 实现计划

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将Android平台的图库文件扫描从MediaStore迁移到Rust FileIndexService，实现与Windows平台完全一致的EXIF优先排序行为。

**Architecture:** 启用现有的Rust FileIndexService在Android上运行，通过新增Tauri命令供前端调用。Kotlin层保留缩略图生成功能但简化数据查询接口。前端统一使用Rust命令替代平台分支逻辑。

**Tech Stack:** Rust (Tauri), Kotlin (Android), TypeScript (React), nom-exif (EXIF解析)

---

## 文件变更清单

### Rust层
- `src-tauri/src/file_index/service.rs` - 移除Android平台限制
- `src-tauri/src/file_index/types.rs` - 添加TS导出(如有需要)
- `src-tauri/src/commands/file_index.rs` - 新增扫描命令
- `src-tauri/src/commands/mod.rs` - 导出新增命令
- `src-tauri/src/lib.rs` - 注册新增命令

### Kotlin层
- `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/GalleryBridge.kt` - 简化接口

### TypeScript层
- `src/types/index.ts` - 更新类型定义
- `src/components/GalleryCard.tsx` - 使用新接口
- `src/components/LatestPhotoCard.tsx` - 使用新接口

---

## Chunk 1: Rust层基础改造

### Task 1.1: 启用Android平台的文件扫描

**Files:**
- Modify: `src-tauri/src/file_index/service.rs`
- Test: 构建验证

- [ ] **Step 1: 移除scan_directory的Android限制**

找到`scan_directory`方法，移除或修改以下代码：

```rust
// 查找并移除 (大约在第187行附近)
#[cfg(not(target_os = "android"))]
pub async fn scan_directory(&self) -> Result<(), AppError> {
```

改为：
```rust
pub async fn scan_directory(&self) -> Result<(), AppError> {
```

- [ ] **Step 2: 验证其他方法的限制**

检查以下方法是否有限制Android的`#[cfg]`属性，如有则移除：
- `scan_recursive`
- `get_file_info`
- `read_exif_time`
- `is_supported_image` (静态方法)

- [ ] **Step 3: 构建验证**

Run: `./build.sh android`

Expected: 构建成功，无新错误

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/file_index/service.rs
git commit -m "feat(file_index): enable file scanning on Android platform

Remove #[cfg(not(target_os = "android"))] restrictions from
scan_directory and related methods to enable file system scanning
on Android platform with MANAGE_EXTERNAL_STORAGE permission."
```

---

### Task 1.2: 新增Tauri命令

**Files:**
- Modify: `src-tauri/src/commands/file_index.rs`
- Test: 构建验证

- [ ] **Step 1: 在file_index.rs末尾添加新命令**

在`handle_file_system_event`命令后添加：

```rust
/// 扫描图库图片（供Android前端调用）
#[command]
pub async fn scan_gallery_images(
    file_index: State<'_, FileIndexService>,
) -> Result<Vec<FileInfo>, AppError> {
    file_index.scan_directory().await?;
    let files = file_index.get_files().await;
    Ok(files.to_vec())
}

/// 获取最新图片（供Android前端调用）
#[command]
pub async fn get_latest_image(
    file_index: State<'_, FileIndexService>,
) -> Result<Option<FileInfo>, AppError> {
    Ok(file_index.get_latest_file().await)
}
```

- [ ] **Step 2: 在commands/mod.rs中导出**

修改`src-tauri/src/commands/mod.rs`，添加导出：

```rust
pub use file_index::{
    get_file_list,
    get_current_file_index,
    navigate_to_file,
    get_latest_file,
    start_file_watcher,
    stop_file_watcher,
    handle_file_system_event,
    scan_gallery_images,  // 新增
    get_latest_image,     // 新增
};
```

- [ ] **Step 3: 在lib.rs中注册命令**

修改`src-tauri/src/lib.rs`，在`invoke_handler`中添加：

```rust
.invoke_handler(tauri::generate_handler![
    // ... 现有命令
    scan_gallery_images,
    get_latest_image,
])
```

- [ ] **Step 4: 构建验证**

Run: `./build.sh android`

Expected: 构建成功

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/file_index.rs
git add src-tauri/src/commands/mod.rs
git add src-tauri/src/lib.rs
git commit -m "feat(commands): add scan_gallery_images and get_latest_image

Add new Tauri commands for Android platform to access file index
with EXIF-based sorting. These commands provide unified interface
for gallery scanning across Windows and Android."
```

---

## Chunk 2: Kotlin层改造

### Task 2.1: 简化GalleryBridge

**Files:**
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/GalleryBridge.kt`
- Test: 构建验证

- [ ] **Step 1: 修改getThumbnail接口签名**

找到`getThumbnail`方法，修改参数类型：

```kotlin
// 修改前
@JavascriptInterface
fun getThumbnail(imageId: Long): String {
    Log.d(TAG, "getThumbnail: imageId=$imageId")
    return try {
        getThumbnailWithCache(imageId)
    } catch (e: Exception) {
        Log.e(TAG, "getThumbnail error for imageId=$imageId", e)
        ""
    }
}
```

改为：
```kotlin
// 修改后
@JavascriptInterface
fun getThumbnail(imagePath: String): String {
    Log.d(TAG, "getThumbnail: imagePath=$imagePath")
    return try {
        getThumbnailWithCache(imagePath)
    } catch (e: Exception) {
        Log.e(TAG, "getThumbnail error for imagePath=$imagePath", e)
        ""
    }
}
```

- [ ] **Step 2: 更新getThumbnailWithCache**

修改私有方法：

```kotlin
// 修改前
private fun getThumbnailWithCache(imageId: Long): String {
    val cacheFile = getThumbnailCacheFile(imageId)
    // ...
    val bitmap = getThumbnailBitmap(imageId) ?: return ""
    // ...
}

// 修改后
private fun getThumbnailWithCache(imagePath: String): String {
    val cacheFile = getThumbnailCacheFile(imagePath)
    // ...
    val bitmap = getThumbnailBitmap(imagePath) ?: return ""
    // ...
}
```

- [ ] **Step 3: 更新getThumbnailBitmap**

```kotlin
// 修改前
private fun getThumbnailBitmap(imageId: Long): Bitmap? {
    // 首先尝试从 MediaStore 获取缓存的缩略图
    val thumbnail = MediaStore.Images.Thumbnails.getThumbnail(
        context.contentResolver,
        imageId,
        MediaStore.Images.Thumbnails.MINI_KIND,
        null
    )
    
    return if (thumbnail != null) {
        thumbnail
    } else {
        // 手动生成缩略图
        createThumbnailManually(imageId)
    }
}

// 修改后
private fun getThumbnailBitmap(imagePath: String): Bitmap? {
    // 直接通过路径生成缩略图，不再依赖MediaStore ID
    val file = File(imagePath)
    if (!file.exists()) return null
    
    return createThumbnailFromFile(file)
}
```

- [ ] **Step 4: 替换createThumbnailManually为createThumbnailFromFile**

```kotlin
/**
 * 从文件生成缩略图
 */
private fun createThumbnailFromFile(file: File): Bitmap? {
    if (!file.exists()) return null

    val options = BitmapFactory.Options().apply {
        inJustDecodeBounds = true
    }
    BitmapFactory.decodeFile(file.absolutePath, options)

    // Calculate sample size for thumbnail
    val sampleSize = calculateSampleSize(
        options.outWidth, 
        options.outHeight, 
        THUMBNAIL_WIDTH, 
        THUMBNAIL_HEIGHT
    )
    options.inJustDecodeBounds = false
    options.inSampleSize = sampleSize

    return BitmapFactory.decodeFile(file.absolutePath, options)
}
```

- [ ] **Step 5: 更新缓存文件命名**

```kotlin
// 修改前
private fun getThumbnailCacheFile(imageId: Long): File {
    return File(getThumbnailCacheDir(), "thumb_$imageId.jpg")
}

// 修改后
private fun getThumbnailCacheFile(imagePath: String): File {
    // 使用路径的MD5作为缓存文件名
    val md5 = imagePath.toByteArray().md5()
    return File(getThumbnailCacheDir(), "thumb_$md5.jpg")
}

// 添加MD5扩展函数
private fun ByteArray.md5(): String {
    val md = java.security.MessageDigest.getInstance("MD5")
    val digest = md.digest(this)
    return digest.joinToString("") { "%02x".format(it) }
}
```

- [ ] **Step 6: 移除不再使用的方法**

删除以下方法：
- `getGalleryImages(storagePath: String): String` - 整方法删除
- `getLatestImage(storagePath: String): String` - 整方法删除
- `getImageSortTime(imageId: Long): Long` - 整方法删除
- `getImagePath(imageId: Long): String?` - 整方法删除
- `createThumbnailManually(imageId: Long): Bitmap?` - 已替换

- [ ] **Step 7: 删除相关导入**

移除不再需要的导入：
```kotlin
// 删除
import android.provider.MediaStore
import android.database.Cursor
```

- [ ] **Step 8: 构建验证**

Run: `./build.sh android`

Expected: 构建成功，GalleryBridge相关错误已解决

- [ ] **Step 9: Commit**

```bash
git add src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/GalleryBridge.kt
git commit -m "refactor(android): simplify GalleryBridge for path-based API

- Change getThumbnail to accept image path instead of MediaStore ID
- Remove getGalleryImages, getLatestImage, getImageSortTime methods
- Update thumbnail caching to use MD5 of path as key
- Direct file access without MediaStore dependency"
```

---

## Chunk 3: TypeScript层改造

### Task 3.1: 更新类型定义

**Files:**
- Modify: `src/types/index.ts`
- Test: TypeScript编译

- [ ] **Step 1: 更新GalleryImage接口**

修改`src/types/index.ts`：

```typescript
// 修改前
export interface GalleryImage {
    id: number;              // MediaStore ID
    path: string;
    filename: string;
    dateModified: number;
    sortTime: number;
}

// 修改后
export interface GalleryImage {
    path: string;            // 完整文件路径（作为主键）
    filename: string;
    sortTime: number;        // EXIF优先的排序时间
}
```

- [ ] **Step 2: 更新FileInfo类型（如有需要）**

确保FileInfo与Rust结构一致：

```typescript
export interface FileInfo {
    path: string;
    filename: string;
    sortTime: number;
}
```

- [ ] **Step 3: 更新全局类型声明**

修改`src/types/global.ts`中GalleryAndroid接口：

```typescript
// 修改前
interface GalleryAndroid {
    getGalleryImages: (storagePath: string) => Promise<string>;
    getThumbnail: (imageId: number) => Promise<string>;
    getLatestImage: (storagePath: string) => Promise<string>;
    getImageSortTime: (imageId: number) => Promise<number>;
    deleteImages: (idsJson: string) => Promise<boolean>;
    shareImages: (idsJson: string) => Promise<boolean>;
}

// 修改后
interface GalleryAndroid {
    getThumbnail: (imagePath: string) => Promise<string>;
    deleteImages: (idsJson: string) => Promise<boolean>;
    shareImages: (idsJson: string) => Promise<boolean>;
}
```

- [ ] **Step 4: TypeScript编译验证**

Run: `npx tsc --noEmit`

Expected: 无类型错误

- [ ] **Step 5: Commit**

```bash
git add src/types/
git commit -m "types: update GalleryImage to use path as ID

- Change GalleryImage.id from number to using path as identifier
- Update GalleryAndroid interface to remove deprecated methods
- Align TypeScript types with new Rust/Kotlin API"
```

---

### Task 3.2: 重构GalleryCard组件

**Files:**
- Modify: `src/components/GalleryCard.tsx`
- Test: 功能验证

- [ ] **Step 1: 修改图片加载逻辑**

替换`loadImages`函数：

```typescript
// 修改前
const loadImages = useCallback(async () => {
    if (!config?.savePath || !window.GalleryAndroid) {
        return;
    }

    setIsLoading(true);
    setError(null);

    try {
        // Load only metadata (fast, no thumbnails)
        const result = await window.GalleryAndroid.getGalleryImages(config.savePath);
        const response = JSON.parse(result) as { images: GalleryImage[] };
        setImages(response.images);
    } catch (err) {
        setError(err instanceof Error ? err.message : 'Failed to load images');
        setImages([]);
    } finally {
        setIsLoading(false);
    }
}, [config?.savePath]);

// 修改后
const loadImages = useCallback(async () => {
    if (!window.GalleryAndroid) {
        return;
    }

    setIsLoading(true);
    setError(null);

    try {
        // 统一使用Rust命令
        const files = await invoke<FileInfo[]>('scan_gallery_images');
        // 转换FileInfo到GalleryImage格式
        const galleryImages: GalleryImage[] = files.map(file => ({
            path: file.path,
            filename: file.filename,
            sortTime: new Date(file.sortTime).getTime(),
        }));
        setImages(galleryImages);
    } catch (err) {
        setError(err instanceof Error ? err.message : 'Failed to load images');
        setImages([]);
    } finally {
        setIsLoading(false);
    }
}, []);
```

- [ ] **Step 2: 修改缩略图加载逻辑**

更新`loadThumbnail`函数：

```typescript
// 修改前
const loadThumbnail = useCallback(async (imageId: number) => {
    // ...
    const thumbnailPath = await window.GalleryAndroid?.getThumbnail(imageId);
    // ...
}, []);

// 修改后
const loadThumbnail = useCallback(async (imagePath: string) => {
    // ...
    const thumbnailPath = await window.GalleryAndroid?.getThumbnail(imagePath);
    // ...
}, []);
```

- [ ] **Step 3: 更新图片网格渲染**

修改渲染逻辑中的key和引用：

```typescript
// 修改前
images.map((image) => {
    const thumbnail = thumbnails.get(image.id);
    // ...
    return (
        <div
            key={image.id}
            data-id={image.id}
            // ...
        />
    );
});

// 修改后
images.map((image) => {
    const thumbnail = thumbnails.get(image.path);
    // ...
    return (
        <div
            key={image.path}
            data-path={image.path}
            // ...
        />
    );
});
```

- [ ] **Step 4: 更新IntersectionObserver处理**

```typescript
// 修改前
const id = Number(entry.target.getAttribute('data-id'));

// 修改后
const path = entry.target.getAttribute('data-path');
```

- [ ] **Step 5: 更新选择相关逻辑**

```typescript
// selectedIds从Set<number>改为Set<string>
const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());

// 图片点击处理
const handleImageClick = useCallback((image: GalleryImage) => {
    if (isSelectionMode) {
        setSelectedIds(prev => {
            const next = new Set(prev);
            next.has(image.path) ? next.delete(image.path) : next.add(image.path);
            // ...
        });
    }
    // ...
}, [isSelectionMode]);
```

- [ ] **Step 6: 删除操作适配**

删除操作需要适配新的ID格式：

```typescript
// 如果deleteImages仍需要ID列表，需要额外处理
// 可能需要在Rust层添加基于路径的删除命令
// 或在前端维护路径到可删除标识的映射
```

- [ ] **Step 7: 导入更新**

```typescript
// 添加
import { invoke } from '@tauri-apps/api/core';
import type { FileInfo } from '../types';
```

- [ ] **Step 8: Commit**

```bash
git add src/components/GalleryCard.tsx
git commit -m "refactor(GalleryCard): use Rust commands for image loading

- Replace MediaStore-based getGalleryImages with scan_gallery_images
- Update thumbnail loading to use image path as identifier
- Migrate from number IDs to string paths for selection and caching"
```

---

### Task 3.3: 重构LatestPhotoCard组件

**Files:**
- Modify: `src/components/LatestPhotoCard.tsx`
- Test: 功能验证

- [ ] **Step 1: 简化最新图片获取逻辑**

替换整个获取逻辑：

```typescript
// 修改前 - 有平台分支
useEffect(() => {
    const fetchLatestFile = async () => {
        // Android: 使用 MediaStore 获取最新图片
        if (window.GalleryAndroid?.getLatestImage && config?.savePath) {
            try {
                const result = await window.GalleryAndroid.getLatestImage(config.savePath);
                if (result && result !== 'null') {
                    const latest = JSON.parse(result) as FileInfo;
                    setScannedLatestFile(latest);
                }
            } catch (err) {
                console.error('[LatestPhotoCard] getLatestImage failed:', err);
            }
            return;
        }

        // Windows: 使用 Rust 文件索引
        try {
            const latest = await invoke<FileInfo | null>('get_latest_file');
            setScannedLatestFile(latest);
        } catch {
            // Silently ignore
        }
    };

    fetchLatestFile();
}, [config?.savePath]);

// 修改后 - 统一使用Rust
useEffect(() => {
    const fetchLatestFile = async () => {
        try {
            const latest = await invoke<FileInfo | null>('get_latest_image');
            setScannedLatestFile(latest);
        } catch (err) {
            console.error('[LatestPhotoCard] Failed to fetch latest image:', err);
        }
    };

    fetchLatestFile();
}, []);
```

- [ ] **Step 2: 更新文件打开逻辑**

```typescript
// 修改前
const handleOpenPreview = useCallback(async () => {
    if (!config?.savePath) return;

    // Android: 使用 MediaStore 实时获取最新图片
    if (window.GalleryAndroid?.getLatestImage) {
        try {
            const result = await window.GalleryAndroid.getLatestImage(config.savePath);
            // ...
        } catch {
            // Silently ignore
        }
        return;
    }

    // Windows: 使用 Rust 文件索引
    // ...
}, [stats.lastFile, scannedLatestFile, config?.savePath]);

// 修改后
const handleOpenPreview = useCallback(async () => {
    try {
        const latest = await invoke<FileInfo | null>('get_latest_image');
        if (latest) {
            setScannedLatestFile(latest);
            // 打开图片...
            if (window.GalleryAndroid) {
                window.PermissionAndroid?.openImageWithChooser(latest.path);
            } else {
                await invoke('open_preview_window', { filePath: latest.path });
            }
        }
    } catch {
        // Silently ignore
    }
}, []);
```

- [ ] **Step 3: 更新事件监听**

```typescript
// 修改前
useEffect(() => {
    // Android使用MediaStore，不需要监听Rust文件索引变化
    if (window.GalleryAndroid) {
        return;
    }

    const unlistenPromise = listen<FileIndexChangedEvent>('file-index-changed', (event) => {
        // ...
    });

    return () => {
        unlistenPromise.then((unlisten) => unlisten()).catch(() => {});
    };
}, []);

// 修改后 - 统一监听（现在Android也使用Rust索引）
useEffect(() => {
    const unlistenPromise = listen<FileIndexChangedEvent>('file-index-changed', (event) => {
        if (event.payload.count === 0) {
            setScannedLatestFile(null);
        } else {
            invoke<FileInfo | null>('get_latest_image')
                .then((latest) => {
                    setScannedLatestFile(latest);
                })
                .catch(() => {
                    // Silently ignore
                });
        }
    });

    return () =㹜 {
        unlistenPromise.then((unlisten) =㹜 unlisten()).catch(() =㹜 {});
    };
}, []);
```

- [ ] **Step 4: Commit**

```bash
git add src/components/LatestPhotoCard.tsx
git commit -m "refactor(LatestPhotoCard): use unified Rust API

- Remove platform-specific branches for fetching latest image
- Use get_latest_image command for both Windows and Android
- Enable file-index-changed event handling on Android"
```

---

## Chunk 4: 集成测试与验证

### Task 4.1: 构建验证

**Files:**
- All modified files
- Test: 完整构建

- [ ] **Step 1: Android构建**

Run: `./build.sh android`

Expected: 构建成功，无错误

- [ ] **Step 2: Windows构建**

Run: `./build.sh windows`

Expected: 构建成功，无新警告

- [ ] **Step 3: 类型检查**

Run: `npx tsc --noEmit`

Expected: 无类型错误

- [ ] **Step 4: Commit (如有必要)**

```bash
git commit -m "chore: fix any build issues" || echo "No changes to commit"
```

---

### Task 4.2: 功能测试

**Files:**
- APK/安装包
- Test: 真机测试

- [ ] **Step 1: 安装测试APK**

将构建好的APK安装到Android设备：

```bash
adb install -r src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release.apk
```

- [ ] **Step 2: 验证图库加载**

测试步骤：
1. 打开APP
2. 进入图库页面
3. 验证图片列表加载
4. 验证排序顺序（EXIF优先）

Expected: 图片按EXIF时间排序，最新的在前

- [ ] **Step 3: 验证刷新功能**

测试步骤：
1. 通过其他方式添加/删除图片
2. 点击刷新按钮
3. 验证列表更新

Expected: 列表正确反映文件系统变化

- [ ] **Step 4: 验证FTP上传**

测试步骤：
1. 启动FTP服务器
2. 上传新图片
3. 验证图片自动出现在列表最前端

Expected: 新图片立即出现在列表顶部

- [ ] **Step 5: 验证缩略图**

测试步骤：
1. 滚动图库
2. 验证缩略图正常加载
3. 验证缩略图缓存有效

Expected: 缩略图正常显示，重复滚动不重新加载

- [ ] **Step 6: 验证最新照片卡片**

测试步骤：
1. 查看首页"最新照片"卡片
2. 上传新图片
3. 验证卡片自动更新

Expected: 卡片实时显示最新图片

---

## 完成检查清单

- [ ] Rust层扫描功能在Android上启用
- [ ] 新增Tauri命令(scan_gallery_images, get_latest_image)
- [ ] Kotlin层GalleryBridge简化完成
- [ ] TypeScript类型定义更新
- [ ] GalleryCard组件使用新API
- [ ] LatestPhotoCard组件使用新API
- [ ] Android构建成功
- [ ] Windows构建成功
- [ ] 功能测试通过

---

**计划完成时间**: 约7-8小时  
**计划保存位置**: `docs/superpowers/plans/2026-03-11-android-gallery-scanning-plan.md`
