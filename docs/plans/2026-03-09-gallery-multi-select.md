# 图库多选功能实现计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 为图库添加长按多选、批量删除和分享功能

**Architecture:** 在 GalleryCard.tsx 添加多选状态管理，通过 FAB + 菜单提供操作入口，扩展 GalleryBridge.kt 添加删除和分享方法

**Tech Stack:** React 18, TypeScript, TailwindCSS, Kotlin, Android MediaStore/Intent

---

### Task 1: 更新 TypeScript 类型定义

**Files:**
- Modify: `src/types/global.ts:126-133`

**Step 1: 添加 GalleryAndroid 新方法类型**

在 `GalleryAndroid` 接口中添加 `deleteImages` 和 `shareImages` 方法：

```typescript
interface GalleryAndroid {
  getGalleryImages(storagePath: string): Promise<string>;
  deleteImages(idsJson: string): Promise<boolean>;
  shareImages(idsJson: string): Promise<boolean>;
}
```

**Step 2: 验证类型无错误**

运行: `./build.sh frontend`
预期: 编译成功，无类型错误

**Step 3: 提交**

```bash
git add src/types/global.ts
git commit -m "feat: add deleteImages and shareImages types to GalleryAndroid"
```

---

### Task 2: 实现 Android GalleryBridge 删除方法

**Files:**
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/GalleryBridge.kt`

**Step 1: 添加必要 imports**

在文件顶部 import 区域添加：

```kotlin
import android.content.ContentUris
import android.content.Intent
import android.widget.Toast
```

**Step 2: 添加 deleteImages 方法**

在 `getGalleryImages` 方法后添加：

```kotlin
@android.webkit.JavascriptInterface
fun deleteImages(idsJson: String): Boolean {
    Log.d(TAG, "deleteImages: idsJson=$idsJson")
    
    return try {
        val ids = JSONArray(idsJson).let { json ->
            (0 until json.length()).map { json.getInt(it) }
        }
        
        if (ids.isEmpty()) {
            Log.w(TAG, "deleteImages: no IDs provided")
            return false
        }
        
        val uri = MediaStore.Images.Media.EXTERNAL_CONTENT_URI
        var deletedCount = 0
        
        ids.forEach { id ->
            val contentUri = ContentUris.withAppendedId(uri, id.toLong())
            val deleted = context.contentResolver.delete(contentUri, null, null)
            if (deleted > 0) {
                deletedCount++
                Log.d(TAG, "Deleted image id=$id")
            }
        }
        
        Log.d(TAG, "deleteImages: deleted $deletedCount/${ids.size} images")
        activity.runOnUiThread {
            Toast.makeText(context, "已删除 $deletedCount 张图片", Toast.LENGTH_SHORT).show()
        }
        
        deletedCount > 0
    } catch (e: Exception) {
        Log.e(TAG, "deleteImages error", e)
        activity.runOnUiThread {
            Toast.makeText(context, "删除失败: ${e.message}", Toast.LENGTH_SHORT).show()
        }
        false
    }
}
```

**Step 3: 验证编译**

运行: `./build.sh android`
预期: 编译成功

**Step 4: 提交**

```bash
git add src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/GalleryBridge.kt
git commit -m "feat(android): add deleteImages method to GalleryBridge"
```

---

### Task 3: 实现 Android GalleryBridge 分享方法

**Files:**
- Modify: `src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/GalleryBridge.kt`

**Step 1: 添加 shareImages 方法**

在 `deleteImages` 方法后添加：

```kotlin
@android.webkit.JavascriptInterface
fun shareImages(idsJson: String): Boolean {
    Log.d(TAG, "shareImages: idsJson=$idsJson")
    
    return try {
        val ids = JSONArray(idsJson).let { json ->
            (0 until json.length()).map { json.getInt(it) }
        }
        
        if (ids.isEmpty()) {
            Log.w(TAG, "shareImages: no IDs provided")
            return false
        }
        
        val uris = ids.map { id ->
            ContentUris.withAppendedId(
                MediaStore.Images.Media.EXTERNAL_CONTENT_URI,
                id.toLong()
            )
        }
        
        val intent = if (uris.size == 1) {
            Intent(Intent.ACTION_SEND).apply {
                type = "image/*"
                putExtra(Intent.EXTRA_STREAM, uris[0])
            }
        } else {
            Intent(Intent.ACTION_SEND_MULTIPLE).apply {
                type = "image/*"
                putParcelableArrayListExtra(Intent.EXTRA_STREAM, ArrayList(uris))
            }
        }.apply {
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        }
        
        val chooser = Intent.createChooser(intent, "分享图片")
        chooser.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        context.startActivity(chooser)
        
        Log.d(TAG, "shareImages: shared ${uris.size} images")
        true
    } catch (e: Exception) {
        Log.e(TAG, "shareImages error", e)
        activity.runOnUiThread {
            Toast.makeText(context, "分享失败: ${e.message}", Toast.LENGTH_SHORT).show()
        }
        false
    }
}
```

**Step 2: 验证编译**

运行: `./build.sh android`
预期: 编译成功

**Step 3: 提交**

```bash
git add src-tauri/gen/android/app/src/main/java/com/gjk/cameraftpcompanion/bridges/GalleryBridge.kt
git commit -m "feat(android): add shareImages method to GalleryBridge"
```

---

### Task 4: 添加多选状态和长按处理

**Files:**
- Modify: `src/components/GalleryCard.tsx`

**Step 1: 添加新的 imports 和状态**

在文件顶部的 imports 中添加 `Check, X, Trash2, Share2, MoreVertical` 图标：

```typescript
import { RefreshCw, ImageOff, Loader2, Check, X, Trash2, Share2, MoreVertical } from 'lucide-react';
```

在组件内部，`visibleImages` 状态后添加新状态：

```typescript
const [isSelectionMode, setIsSelectionMode] = useState(false);
const [selectedIds, setSelectedIds] = useState<Set<number>>(new Set());
const [showMenu, setShowMenu] = useState(false);
const menuRef = useRef<HTMLDivElement>(null);
```

**Step 2: 添加长按检测 hook**

在 `imageRefCallback` 后添加长按处理逻辑：

```typescript
const longPressTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
const LONG_PRESS_DURATION = 500;

const handleTouchStart = useCallback((image: GalleryImage) => {
  longPressTimerRef.current = setTimeout(() => {
    setIsSelectionMode(true);
    setSelectedIds(new Set([image.id]));
  }, LONG_PRESS_DURATION);
}, []);

const handleTouchEnd = useCallback(() => {
  if (longPressTimerRef.current) {
    clearTimeout(longPressTimerRef.current);
    longPressTimerRef.current = null;
  }
}, []);
```

**Step 3: 修改 handleImageClick 支持多选模式**

替换原有的 `handleImageClick`：

```typescript
const handleImageClick = useCallback((image: GalleryImage) => {
  if (isSelectionMode) {
    setSelectedIds(prev => {
      const next = new Set(prev);
      next.has(image.id) ? next.delete(image.id) : next.add(image.id);
      if (next.size === 0) {
        setIsSelectionMode(false);
      }
      return next;
    });
  } else if (window.PermissionAndroid?.openImageWithChooser) {
    window.PermissionAndroid.openImageWithChooser(image.path);
  }
}, [isSelectionMode]);
```

**Step 4: 添加操作处理函数**

在 `handleRefresh` 后添加：

```typescript
const handleDelete = useCallback(async () => {
  if (selectedIds.size === 0) return;
  
  if (confirm(`确定删除 ${selectedIds.size} 张图片？`)) {
    const success = await window.GalleryAndroid?.deleteImages(JSON.stringify([...selectedIds]));
    if (success) {
      loadImages();
      setIsSelectionMode(false);
      setSelectedIds(new Set());
      setShowMenu(false);
    }
  }
}, [selectedIds, loadImages]);

const handleShare = useCallback(async () => {
  if (selectedIds.size === 0) return;
  
  await window.GalleryAndroid?.shareImages(JSON.stringify([...selectedIds]));
  setShowMenu(false);
}, [selectedIds]);

const handleCancelSelection = useCallback(() => {
  setIsSelectionMode(false);
  setSelectedIds(new Set());
  setShowMenu(false);
}, []);
```

**Step 5: 添加点击外部关闭菜单**

在 `handleCancelSelection` 后添加：

```typescript
useEffect(() => {
  const handleClickOutside = (event: MouseEvent) => {
    if (menuRef.current && !menuRef.current.contains(event.target as Node)) {
      setShowMenu(false);
    }
  };
  
  if (showMenu) {
    document.addEventListener('mousedown', handleClickOutside);
  }
  
  return () => {
    document.removeEventListener('mousedown', handleClickOutside);
  };
}, [showMenu]);
```

**Step 6: 验证编译**

运行: `./build.sh frontend`
预期: 编译成功

**Step 7: 提交**

```bash
git add src/components/GalleryCard.tsx
git commit -m "feat: add selection state and long-press handler to GalleryCard"
```

---

### Task 5: 实现 UI 组件 - 选中指示器和 FAB

**Files:**
- Modify: `src/components/GalleryCard.tsx`

**Step 1: 修改图片网格项，添加选中指示器**

找到 `images.map` 部分（约 183-204 行），替换整个 map 内容：

```tsx
{images.map((image) => (
  <div
    key={image.id}
    data-id={image.id}
    ref={imageRefCallback}
    onClick={() => handleImageClick(image)}
    onTouchStart={() => handleTouchStart(image)}
    onTouchEnd={handleTouchEnd}
    onTouchCancel={handleTouchEnd}
    className={`aspect-square bg-gray-100 rounded-lg overflow-hidden cursor-pointer hover:opacity-90 transition-opacity relative ${
      isSelectionMode && selectedIds.has(image.id) ? 'ring-2 ring-blue-500' : ''
    }`}
  >
    {visibleImages.has(image.id) ? (
      <img
        src={image.thumbnail}
        alt={image.filename}
        className="w-full h-full object-cover"
        loading="lazy"
      />
    ) : (
      <div className="w-full h-full flex items-center justify-center">
        <div className="w-8 h-8 bg-gray-200 rounded animate-pulse" />
      </div>
    )}
    
    {isSelectionMode && (
      <div className={`absolute top-2 left-2 w-6 h-6 rounded-full flex items-center justify-center ${
        selectedIds.has(image.id)
          ? 'bg-blue-500'
          : 'bg-black/30 border-2 border-white/70'
      }`}>
        {selectedIds.has(image.id) && (
          <Check className="w-4 h-4 text-white" />
        )}
      </div>
    )}
  </div>
))}
```

**Step 2: 验证编译**

运行: `./build.sh frontend`
预期: 编译成功

**Step 3: 提交**

```bash
git add src/components/GalleryCard.tsx
git commit -m "feat: add selection indicator to gallery images"
```

---

### Task 6: 实现 FAB 和操作菜单 UI

**Files:**
- Modify: `src/components/GalleryCard.tsx`

**Step 1: 在 return 语句末尾添加 FAB 和菜单**

找到 `</div>` 结束标签（约 206 行），在其前面添加：

```tsx
      {/* FAB and Menu for selection mode */}
      {isSelectionMode && (
        <div className="fixed bottom-20 right-4 z-50" ref={menuRef}>
          {/* Menu */}
          {showMenu && (
            <div className="absolute bottom-16 right-0 bg-white rounded-xl shadow-xl min-w-[140px] overflow-hidden mb-2">
              <button
                onClick={handleDelete}
                disabled={selectedIds.size === 0}
                className="w-full flex items-center gap-3 px-4 py-3 text-left hover:bg-gray-50 disabled:opacity-50 disabled:cursor-not-allowed"
              >
                <Trash2 className="w-5 h-5 text-red-500" />
                <span>删除({selectedIds.size})</span>
              </button>
              <button
                onClick={handleShare}
                disabled={selectedIds.size === 0}
                className="w-full flex items-center gap-3 px-4 py-3 text-left hover:bg-gray-50 disabled:opacity-50 disabled:cursor-not-allowed border-t border-gray-100"
              >
                <Share2 className="w-5 h-5 text-blue-500" />
                <span>分享({selectedIds.size})</span>
              </button>
              <button
                onClick={handleCancelSelection}
                className="w-full flex items-center gap-3 px-4 py-3 text-left hover:bg-gray-50 border-t border-gray-100"
              >
                <X className="w-5 h-5 text-gray-500" />
                <span>取消选择</span>
              </button>
            </div>
          )}
          
          {/* FAB */}
          <button
            onClick={() => setShowMenu(prev => !prev)}
            className="w-14 h-14 rounded-full bg-blue-500 shadow-lg flex items-center justify-center text-white hover:bg-blue-600 transition-colors"
          >
            <MoreVertical className="w-6 h-6" />
          </button>
          
          {/* Badge */}
          {selectedIds.size > 0 && (
            <div className="absolute -top-1 -right-1 w-6 h-6 rounded-full bg-red-500 text-white text-xs flex items-center justify-center font-medium">
              {selectedIds.size > 99 ? '99+' : selectedIds.size}
            </div>
          )}
        </div>
      )}
```

**Step 2: 验证完整构建**

运行: `./build.sh windows android`
预期: 两个平台都编译成功

**Step 3: 提交**

```bash
git add src/components/GalleryCard.tsx
git commit -m "feat: add FAB and action menu for multi-select operations"
```

---

### Task 7: 最终验证和集成测试

**Step 1: 完整构建验证**

运行: `./build.sh windows android`
预期: 构建成功，无错误

**Step 2: 提交所有变更**

```bash
git add -A
git status
```

确认所有变更已提交。

---

## 完成检查清单

- [ ] TypeScript 类型定义已更新
- [ ] GalleryBridge.kt 添加 deleteImages 方法
- [ ] GalleryBridge.kt 添加 shareImages 方法
- [ ] GalleryCard.tsx 添加多选状态管理
- [ ] GalleryCard.tsx 实现长按进入多选
- [ ] GalleryCard.tsx 添加选中指示器 UI
- [ ] GalleryCard.tsx 添加 FAB 和操作菜单
- [ ] Windows 和 Android 双平台构建通过
