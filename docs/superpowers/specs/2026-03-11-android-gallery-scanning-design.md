# Android图库文件扫描重构设计文档

**日期**: 2026-03-11  
**作者**: Claude  
**状态**: 待实现  
**关联需求**: 统一Android与Windows平台的图库排序行为

---

## 1. 问题陈述

### 1.1 当前问题

Android平台当前使用MediaStore API查询图库文件，存在以下问题：

1. **排序不可靠**: MediaStore使用文件修改时间排序，忽略EXIF拍摄时间
2. **数据不同步**: MediaStore索引可能滞后，导致图片"消失"或顺序错误
3. **平台不一致**: Android和Windows使用完全不同的排序逻辑
4. **行为差异**: 新FTP上传的图片无法保证插入到列表最前端

### 1.2 目标

- 使用与Windows平台完全一致的扫描和排序逻辑
- 排序优先级: EXIF DateTimeOriginal → EXIF DateTime → 文件修改时间
- 新FTP文件始终插入到排序正确位置（通常为最前端）
- 保持缩略图生成在Kotlin层（利用Android原生API优势）

---

## 2. 架构设计

### 2.1 系统架构图

```
┌─────────────────────────────────────────────────────────────────┐
│                           Frontend                              │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────┐  │
│  │  GalleryCard    │  │ LatestPhotoCard │  │  PreviewWindow  │  │
│  └────────┬────────┘  └────────┬────────┘  └────────┬────────┘  │
└───────────┼────────────────────┼────────────────────┼───────────┘
            │                    │                    │
            └────────────────────┴────────────────────┘
                                 │
                    ┌────────────▼────────────┐
                    │   Rust FileIndexService │  ← 统一扫描+排序逻辑
                    │   (Windows & Android)   │
                    └────────────┬────────────┘
                                 │
            ┌────────────────────┼────────────────────┐
            │                    │                    │
     ┌──────▼──────┐    ┌───────▼────────┐   ┌──────▼──────┐
     │  Windows    │    │     Android    │   │   Android   │
     │ notify-rs   │    │ FileObserver   │   │ Thumbnail   │
     │ (可选监听)   │    │ (保留，暂不    │   │ (Kotlin层   │
     │             │    │  移除)         │   │  保留)      │
     └─────────────┘    └────────────────┘   └─────────────┘
```

### 2.2 核心组件

#### 2.2.1 Rust层 - FileIndexService

**位置**: `src-tauri/src/file_index/service.rs`

**职责**:
- 递归扫描指定目录
- 读取图片EXIF元数据
- 按优先级计算排序时间
- 维护已排序的文件列表
- 处理FTP上传事件的增量更新

**关键方法**:
```rust
impl FileIndexService {
    /// 扫描目录（现在支持Android）
    pub async fn scan_directory(&self) -> Result<(), AppError>;
    
    /// 读取EXIF时间
    async fn read_exif_time(&self, path: &Path) -> Option<SystemTime>;
    
    /// 添加新文件（FTP上传时调用）
    pub async fn add_file(&self, path: PathBuf) -> Result<(), AppError>;
    
    /// 获取排序后的文件列表
    pub async fn get_files(&self) -> Arc<Vec<FileInfo>>;
    
    /// 获取最新文件
    pub async fn get_latest_file(&self) -> Option<FileInfo>;
}
```

**排序逻辑**:
```rust
let exif_time = self.read_exif_time(path).await;
let sort_time = exif_time.unwrap_or(modified_time);
```

#### 2.2.2 Rust层 - 新增Tauri命令

**位置**: `src-tauri/src/commands/file_index.rs`

**新增命令**:

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

**命令注册**:
在 `src-tauri/src/lib.rs` 的 `invoke_handler` 中添加:
```rust
.invoke_handler(tauri::generate_handler![
    // ... 现有命令
    scan_gallery_images,
    get_latest_image,
])
```

#### 2.2.3 Kotlin层 - GalleryBridge（简化版）

**位置**: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/GalleryBridge.kt`

**保留功能**:
- `getThumbnail(imagePath: String): String` - 生成/返回缩略图
- `deleteImages(idsJson: String): Boolean` - 删除图片
- `shareImages(idsJson: String): Boolean` - 分享图片
- 缩略图缓存管理

**移除功能**:
- `getGalleryImages(storagePath: String): String` - 由Rust接管
- `getLatestImage(storagePath: String): String` - 由Rust接管
- `getImageSortTime(imageId: Long): Long` - 由Rust在扫描时处理

**接口变更**:
```kotlin
// 修改前
@JavascriptInterface
fun getThumbnail(imageId: Long): String

// 修改后
@JavascriptInterface
fun getThumbnail(imagePath: String): String
```

#### 2.2.4 前端层 - GalleryCard.tsx

**位置**: `src/components/GalleryCard.tsx`

**变更要点**:
1. 移除Android特有代码路径
2. 统一使用Rust命令
3. 图片ID从`number`改为`string`（路径）

**关键代码变更**:
```typescript
// 修改前：Android特有
const result = await window.GalleryAndroid?.getGalleryImages(config.savePath);
const response = JSON.parse(result) as { images: GalleryImage[] };

// 修改后：统一使用Rust
const files = await invoke<FileInfo[]>('scan_gallery_images');

// 修改前：使用MediaStore ID
const thumbnail = await window.GalleryAndroid?.getThumbnail(image.id);

// 修改后：使用路径
const thumbnail = await window.GalleryAndroid?.getThumbnail(image.path);
```

#### 2.2.5 前端层 - LatestPhotoCard.tsx

**位置**: `src/components/LatestPhotoCard.tsx`

**变更要点**:
1. 移除平台分支逻辑
2. 统一使用`get_latest_image`命令

**关键代码变更**:
```typescript
// 修改前：Android特有分支
if (window.GalleryAndroid?.getLatestImage && config?.savePath) {
    const result = await window.GalleryAndroid.getLatestImage(config.savePath);
    // ...
}

// 修改后：统一使用Rust
const latest = await invoke<FileInfo | null>('get_latest_image');
```

---

## 3. 数据流设计

### 3.1 初始加载/手动刷新

```
用户打开APP / 点击刷新按钮
    ↓
前端: invoke('scan_gallery_images')
    ↓
Rust FileIndexService:
    ├── 递归遍历目录 (tokio::fs::read_dir)
    ├── 筛选支持的图片格式
    ├── 对每个文件:
    │   ├── 读取文件元数据
    │   ├── 读取EXIF时间 (nom-exif)
    │   └── 计算 sort_time (EXIF优先)
    ├── 按 sort_time 降序排序
    └── 存储到内存索引
    ↓
返回 Vec<FileInfo>
    ↓
前端渲染图库网格
```

### 3.2 FTP新文件上传

```
FTP服务器接收到新文件
    ↓
FtpDataListener 捕获 DataEvent::Put
    ↓
检查是否为支持的图片格式
    ↓
等待文件完全写入 (wait_for_file_ready)
    ↓
调用 FileIndexService::add_file(path)
    ├── 读取文件元数据
    ├── 读取EXIF时间
    ├── 计算 sort_time
    ├── 二分查找插入位置 (保持排序)
    └── 插入到正确位置
    ↓
emit 'file-index-changed' 事件
    ↓
前端自动更新显示
```

### 3.3 缩略图加载

```
图片滚动到可视区域
    ↓
IntersectionObserver 触发
    ↓
前端调用: window.GalleryAndroid?.getThumbnail(image.path)
    ↓
Kotlin GalleryBridge:
    ├── 检查缩略图缓存是否存在且有效
    ├── 缓存命中: 直接返回缓存路径
    └── 缓存未命中:
        ├── 使用 BitmapFactory.decodeFile 生成缩略图
        ├── 压缩到 THUMBNAIL_WIDTH x THUMBNAIL_HEIGHT
        ├── 保存到缓存目录
        └── 返回缓存路径
    ↓
前端使用 convertFileSrc() 转换为 asset:// URL 显示
```

---

## 4. 数据模型

### 4.1 Rust - FileInfo

**位置**: `src-tauri/src/file_index/types.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct FileInfo {
    pub path: PathBuf,              // 完整文件路径（作为主键）
    pub filename: String,           // 文件名（显示用）
    #[ts(skip)]
    pub exif_time: Option<SystemTime>,  // EXIF时间（可能不存在）
    #[ts(skip)]
    pub modified_time: SystemTime,  // 文件修改时间
    #[ts(skip)]
    pub sort_time: SystemTime,      // 实际用于排序的时间
}
```

### 4.2 TypeScript - GalleryImage

**位置**: `src/types/index.ts` (需要更新)

```typescript
export interface GalleryImage {
    path: string;           // 完整文件路径（替代原来的number ID）
    filename: string;
    sortTime: number;       // 时间戳（毫秒）
}
```

### 4.3 ID变更对比

| 平台 | 修改前 | 修改后 |
|------|--------|--------|
| Windows | `PathBuf` (路径) | `PathBuf` (路径) - 不变 |
| Android | `number` (MediaStore ID) | `string` (路径) |

---

## 5. 排序优先级

### 5.1 排序时间计算

```rust
fn calculate_sort_time(exif_time: Option<SystemTime>, modified_time: SystemTime) -> SystemTime {
    // 优先级1: EXIF DateTimeOriginal（拍摄时间）
    // 优先级2: EXIF DateTime（数字化时间）
    // 优先级3: 文件修改时间（mtime）
    exif_time.unwrap_or(modified_time)
}
```

### 5.2 排序规则

- **降序排列**: 最新的图片排在最前面
- **稳定排序**: 相同sort_time的文件保持原始顺序
- **实时更新**: FTP新文件插入到正确位置，而非简单追加

---

## 6. 接口变更清单

### 6.1 新增接口

#### Rust Tauri命令

| 命令名 | 参数 | 返回值 | 用途 |
|--------|------|--------|------|
| `scan_gallery_images` | 无 | `Vec<FileInfo>` | 扫描并返回所有图片 |
| `get_latest_image` | 无 | `Option<FileInfo>` | 获取最新图片 |

### 6.2 修改接口

#### Kotlin GalleryBridge

| 方法 | 修改前 | 修改后 |
|------|--------|--------|
| `getThumbnail` | `getThumbnail(imageId: Long)` | `getThumbnail(imagePath: String)` |

**注意**: `getGalleryImages`、`getLatestImage`、`getImageSortTime`将被移除

### 6.3 移除接口

#### Kotlin GalleryBridge

- `getGalleryImages(storagePath: String): String`
- `getLatestImage(storagePath: String): String`
- `getImageSortTime(imageId: Long): Long`

---

## 7. 错误处理

### 7.1 扫描错误

| 场景 | 处理方式 |
|------|----------|
| 目录不存在 | 返回空列表，记录warning日志 |
| 无权限读取文件 | 跳过该文件，继续扫描其他 |
| EXIF读取失败 | 使用文件修改时间作为fallback |
| 文件格式不支持 | 跳过，不加入列表 |

### 7.2 缩略图错误

| 场景 | 处理方式 |
|------|----------|
| 文件不存在 | 返回空字符串，前端显示占位图 |
| 解码失败 | 尝试Base64回退，否则返回空 |
| 缓存写入失败 | 返回Base64编码的图片数据 |

---

## 8. 性能考虑

### 8.1 扫描性能

- **首次扫描**: O(n) 遍历目录，n为文件数量
- **EXIF读取**: 使用`nom-exif`快速解析，支持JPG/RAW/CR3/NEF等格式
- **并发处理**: 扫描过程使用异步IO，不阻塞主线程

### 8.2 缩略图性能

- **LRU缓存**: 100MB磁盘缓存，自动清理旧文件
- **懒加载**: 仅当图片进入可视区域时才加载缩略图
- **批量加载**: IntersectionObserver批量触发，减少重渲染

### 8.3 内存使用

- **索引存储**: 仅存储元数据（路径+时间），不加载图片数据
- **缩略图**: 磁盘缓存，内存中仅保留可见项的Bitmap

---

## 9. 测试策略

### 9.1 单元测试

**Rust层**:
- `read_exif_time` 的各种输入情况
- `calculate_sort_time` 的优先级逻辑
- `scan_directory` 的目录遍历

**Kotlin层**:
- 缩略图生成和缓存
- 文件路径到Bitmap的转换

### 9.2 集成测试

- 扫描包含EXIF/无EXIF的混合目录
- FTP上传后文件列表自动更新
- 手动刷新后数据一致性

### 9.3 手动测试清单

- [ ] APP启动时正确显示图库
- [ ] 点击刷新按钮更新列表
- [ ] FTP上传后新图片出现在列表最前端
- [ ] EXIF时间与文件时间不同的图片按EXIF排序
- [ ] 缩略图正常加载和显示
- [ ] 删除图片后列表更新
- [ ] 最新照片卡片显示正确

---

## 10. 实施计划

### 10.1 阶段划分

**阶段1: Rust层改造** (预计2小时)
1. 移除`scan_directory`的`#[cfg]`限制
2. 新增Tauri命令
3. 注册命令到lib.rs

**阶段2: Kotlin层简化** (预计1.5小时)
1. 修改`getThumbnail`接口
2. 移除`getGalleryImages`等方法
3. 更新缓存key生成逻辑

**阶段3: 前端适配** (预计1.5小时)
1. 更新GalleryCard.tsx
2. 更新LatestPhotoCard.tsx
3. 更新类型定义

**阶段4: 测试验证** (预计2小时)
1. Android构建测试
2. 功能验证
3. 性能测试

### 10.2 依赖关系

```
Rust层改造
    ↓
Kotlin层简化
    ↓
前端适配
    ↓
测试验证
```

---

## 11. 风险评估

### 11.1 潜在风险

| 风险 | 可能性 | 影响 | 缓解措施 |
|------|--------|------|----------|
| Android文件系统权限问题 | 低 | 高 | 已持有MANAGE_EXTERNAL_STORAGE |
| EXIF库在Android上异常 | 低 | 中 | nom-exif是纯Rust实现，跨平台兼容 |
| 大量文件扫描性能问题 | 中 | 中 | 异步处理+懒加载，分批优化 |
| 前端类型不匹配 | 中 | 低 | TypeScript严格类型检查 |

### 11.2 回滚计划

如遇到严重问题，可快速回滚到MediaStore方案：
1. 恢复Kotlin层的`getGalleryImages`方法
2. 前端恢复平台分支逻辑
3. Rust命令保留（不影响原有功能）

---

## 12. 附录

### 12.1 相关文件清单

**Rust**:
- `src-tauri/src/file_index/service.rs`
- `src-tauri/src/file_index/types.rs`
- `src-tauri/src/commands/file_index.rs`
- `src-tauri/src/lib.rs`

**Kotlin**:
- `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/GalleryBridge.kt`

**TypeScript**:
- `src/components/GalleryCard.tsx`
- `src/components/LatestPhotoCard.tsx`
- `src/types/index.ts`

### 12.2 参考文档

- [Tauri v2 Documentation](https://tauri.app/)
- [nom-exif Crate](https://docs.rs/nom-exif/)
- [Android File System Access](https://developer.android.com/training/data-storage/manage-all-files)

---

**文档版本**: 1.0  
**最后更新**: 2026-03-11
