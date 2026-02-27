# 图片导航功能实现计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 实现内置预览窗口的图片导航功能，支持浏览存储路径下的所有图片（按EXIF时间排序），添加上/下一张、最旧/最新导航按钮和快捷键。

**Architecture:** 
- 后端新增 `FileIndexService` 管理文件列表（扫描、EXIF读取、排序、实时更新）
- 前端 `PreviewWindow` 添加导航按钮和键盘事件监听
- 更新 `StatsCard` "最新照片"按钮逻辑

**Tech Stack:** Rust (tauri, kamadak-exif), React (TypeScript), Zustand

---

## Task 1: 添加 EXIF 读取依赖

**Files:**
- Modify: `src-tauri/Cargo.toml`

**Step 1: 添加 EXIF 库依赖**

```toml
[dependencies]
# ... 现有依赖 ...
kamadak-exif = "0.5"
```

**Step 2: 编译验证**

Run: `cd src-tauri && cargo check`
Expected: 成功，无错误

**Step 3: Commit**

```bash
git add src-tauri/Cargo.toml
git commit -m "deps: add kamadak-exif for reading image EXIF data"
```

---

## Task 2: 创建 FileIndexService 模块

**Files:**
- Create: `src-tauri/src/file_index/mod.rs`
- Create: `src-tauri/src/file_index/service.rs`
- Create: `src-tauri/src/file_index/types.rs`
- Modify: `src-tauri/src/lib.rs` (添加模块声明)

**Step 1: 创建类型定义**

```rust
// src-tauri/src/file_index/types.rs
use std::path::PathBuf;
use std::time::SystemTime;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct FileInfo {
    pub path: PathBuf,
    pub filename: String,
    pub exif_time: Option<SystemTime>,
    pub modified_time: SystemTime,
    pub sort_time: SystemTime, // 优先使用 exif_time，不存在则使用 modified_time
}

#[derive(Debug, Clone)]
pub struct FileIndex {
    pub files: Vec<FileInfo>,
    pub current_index: Option<usize>,
}

impl FileIndex {
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            current_index: None,
        }
    }
}
```

**Step 2: 创建服务实现**

```rust
// src-tauri/src/file_index/service.rs
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tokio::sync::RwLock;
use tracing::{info, warn, error};

use crate::config::AppConfig;
use crate::error::AppError;
use super::types::{FileIndex, FileInfo};

pub struct FileIndexService {
    index: RwLock<FileIndex>,
    save_path: PathBuf,
}

impl FileIndexService {
    pub fn new() -> Self {
        let config = AppConfig::load();
        Self {
            index: RwLock::new(FileIndex::new()),
            save_path: config.save_path,
        }
    }

    /// 扫描目录建立索引
    pub async fn scan_directory(&self) -> Result<(), AppError> {
        info!("Starting directory scan: {:?}", self.save_path);
        
        let mut files = Vec::new();
        self.scan_recursive(&self.save_path, &mut files).await?;
        
        // 按 sort_time 排序（新→旧）
        files.sort_by(|a, b| b.sort_time.cmp(&a.sort_time));
        
        let mut index = self.index.write().await;
        index.files = files;
        index.current_index = index.files.first().map(|_| 0);
        
        info!("Directory scan complete: {} files found", index.files.len());
        Ok(())
    }

    /// 递归扫描目录
    async fn scan_recursive(&self, dir: &Path, files: &mut Vec<FileInfo>) -> Result<(), AppError> {
        let mut entries = tokio::fs::read_dir(dir).await
            .map_err(|e| AppError::Other(format!("Failed to read dir: {}", e)))?;

        while let Some(entry) = entries.next_entry().await
            .map_err(|e| AppError::Other(format!("Failed to read entry: {}", e)))? 
        {
            let path = entry.path();
            let metadata = entry.metadata().await;
            
            if metadata.is_err() {
                continue; // 跳过无权限文件
            }
            let metadata = metadata.unwrap();
            
            if metadata.is_dir() {
                // 递归扫描子目录
                if let Err(e) = self.scan_recursive(&path, files).await {
                    warn!("Failed to scan subdirectory {:?}: {}", path, e);
                }
            } else if metadata.is_file() {
                // 检查是否是支持的图片格式
                if Self::is_supported_image(&path) {
                    match self.get_file_info(&path, &metadata).await {
                        Ok(file_info) => files.push(file_info),
                        Err(e) => warn!("Failed to get file info for {:?}: {}", path, e),
                    }
                }
            }
        }
        
        Ok(())
    }

    /// 检查文件是否是支持的图片格式
    fn is_supported_image(path: &Path) -> bool {
        let ext = path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();
        
        matches!(ext.as_str(), "jpg" | "jpeg" | "heif" | "hif" | "heic")
    }

    /// 获取文件信息（包括EXIF时间）
    async fn get_file_info(&self, path: &Path, metadata: &std::fs::Metadata) -> Result<FileInfo, AppError> {
        let filename = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        
        let modified_time = metadata.modified()
            .unwrap_or_else(|_| SystemTime::UNIX_EPOCH);
        
        // 尝试读取EXIF时间
        let exif_time = self.read_exif_time(path).await;
        
        // sort_time 优先使用 exif_time
        let sort_time = exif_time.unwrap_or(modified_time);
        
        Ok(FileInfo {
            path: path.to_path_buf(),
            filename,
            exif_time,
            modified_time,
            sort_time,
        })
    }

    /// 读取图片EXIF中的拍摄时间
    async fn read_exif_time(&self, path: &Path) -> Option<SystemTime> {
        // 使用 spawn_blocking 因为 EXIF 读取是同步操作
        let path = path.to_path_buf();
        tokio::task::spawn_blocking(move || {
            let file = std::fs::File::open(&path).ok()?;
            let mut bufreader = std::io::BufReader::new(file);
            let exifreader = exif::Reader::new();
            let exif = exifreader.read_from_container(&mut bufreader).ok()?;
            
            // 优先读取 DateTimeOriginal，不存在则读取 DateTime
            let datetime_field = exif.get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY)
                .or_else(|| exif.get_field(exif::Tag::DateTime, exif::In::PRIMARY))?;
            
            let datetime_str = datetime_field.display_value().with_unit(&exif).to_string();
            
            // 解析 EXIF 时间格式: "2024:02:26 14:30:00"
            Self::parse_exif_datetime(&datetime_str)
        }).await.ok()?
    }

    /// 解析 EXIF 时间字符串
    fn parse_exif_datetime(datetime_str: &str) -> Option<SystemTime> {
        // 格式: "2024:02:26 14:30:00"
        let parts: Vec<&str> = datetime_str.split(&[':', ' ', '-']).collect();
        if parts.len() >= 6 {
            let year = parts[0].parse::<i32>().ok()?;
            let month = parts[1].parse::<u32>().ok()?;
            let day = parts[2].parse::<u32>().ok()?;
            let hour = parts[3].parse::<u32>().ok()?;
            let minute = parts[4].parse::<u32>().ok()?;
            let second = parts[5].parse::<u32>().ok()?;
            
            use std::time::{SystemTime, Duration};
            // 简化为从 UNIX_EPOCH 开始的秒数计算（简化版）
            // 实际应该使用 chrono 库进行精确计算
            // 这里为了演示使用近似值
            let days_since_epoch = (year - 1970) as u64 * 365 + (month - 1) as u64 * 30 + day as u64;
            let seconds = days_since_epoch * 24 * 3600 + hour as u64 * 3600 + minute as u64 * 60 + second as u64;
            Some(SystemTime::UNIX_EPOCH + Duration::from_secs(seconds))
        } else {
            None
        }
    }

    /// 添加新文件（FTP上传时调用）
    pub async fn add_file(&self, path: PathBuf) -> Result<(), AppError> {
        if !Self::is_supported_image(&path) {
            return Ok(()); // 跳过非图片文件
        }

        let metadata = tokio::fs::metadata(&path).await
            .map_err(|e| AppError::Other(format!("Failed to get metadata: {}", e)))?;
        
        let file_info = self.get_file_info(&path, &metadata).await?;
        
        let mut index = self.index.write().await;
        
        // 插入到正确位置（保持排序）
        let insert_pos = index.files.iter()
            .position(|f| f.sort_time < file_info.sort_time)
            .unwrap_or(index.files.len());
        
        index.files.insert(insert_pos, file_info);
        
        // 更新 current_index 如果插入位置在 current_index 之前
        if let Some(current) = index.current_index {
            if insert_pos <= current {
                index.current_index = Some(current + 1);
            }
        }
        
        info!("Added file to index: {:?}", path);
        Ok(())
    }

    /// 获取文件列表
    pub async fn get_files(&self) -> Vec<FileInfo> {
        let index = self.index.read().await;
        index.files.clone()
    }

    /// 获取当前索引
    pub async fn get_current_index(&self) -> Option<usize> {
        let index = self.index.read().await;
        index.current_index
    }

    /// 导航到指定索引
    pub async fn navigate_to(&self, new_index: usize) -> Result<FileInfo, AppError> {
        let index = self.index.read().await;
        
        if new_index >= index.files.len() {
            return Err(AppError::Other("Index out of bounds".to_string()));
        }
        
        let file_info = index.files[new_index].clone();
        drop(index); // 释放读锁
        
        // 更新当前索引
        let mut index = self.index.write().await;
        index.current_index = Some(new_index);
        
        Ok(file_info)
    }

    /// 获取最新文件（排序第一个）
    pub async fn get_latest_file(&self) -> Option<FileInfo> {
        let index = self.index.read().await;
        index.files.first().cloned()
    }

    /// 获取文件数量
    pub async fn get_file_count(&self) -> usize {
        let index = self.index.read().await;
        index.files.len()
    }
}

impl Default for FileIndexService {
    fn default() -> Self {
        Self::new()
    }
}
```

**Step 3: 创建模块导出**

```rust
// src-tauri/src/file_index/mod.rs
mod service;
mod types;

pub use service::FileIndexService;
pub use types::{FileIndex, FileInfo};
```

**Step 4: 添加到 lib.rs**

```rust
// src-tauri/src/lib.rs
mod file_index;

use file_index::FileIndexService;
```

**Step 5: 编译验证**

Run: `cd src-tauri && cargo check`
Expected: 成功，可能有一些未使用警告

**Step 6: Commit**

```bash
git add src-tauri/src/file_index/
git add src-tauri/src/lib.rs
git commit -m "feat: add FileIndexService for scanning and indexing images"
```

---

## Task 3: 添加 Tauri 命令

**Files:**
- Modify: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs` (注册命令)

**Step 1: 添加文件索引相关命令**

```rust
// src-tauri/src/commands.rs

use crate::file_index::{FileIndexService, FileInfo};

/// 获取文件列表
#[tauri::command]
pub async fn get_file_list(
    file_index: State<'_, FileIndexService>,
) -> Result<Vec<FileInfo>, AppError> {
    Ok(file_index.get_files().await)
}

/// 获取当前文件索引
#[tauri::command]
pub async fn get_current_file_index(
    file_index: State<'_, FileIndexService>,
) -> Result<Option<usize>, AppError> {
    Ok(file_index.get_current_index().await)
}

/// 导航到指定索引
#[tauri::command]
pub async fn navigate_to_file(
    file_index: State<'_, FileIndexService>,
    index: usize,
) -> Result<FileInfo, AppError> {
    file_index.navigate_to(index).await
}

/// 获取最新文件
#[tauri::command]
pub async fn get_latest_file(
    file_index: State<'_, FileIndexService>,
) -> Result<Option<FileInfo>, AppError> {
    Ok(file_index.get_latest_file().await)
}
```

**Step 2: 注册命令和状态**

```rust
// src-tauri/src/lib.rs

.manage(FileIndexService::new())

.invoke_handler(tauri::generate_handler![
    // ... 现有命令 ...
    get_file_list,
    get_current_file_index,
    navigate_to_file,
    get_latest_file,
])
```

**Step 3: 编译验证**

Run: `cd src-tauri && cargo check`
Expected: 成功

**Step 4: Commit**

```bash
git add src-tauri/src/commands.rs
git add src-tauri/src/lib.rs
git commit -m "feat: add file index Tauri commands"
```

---

## Task 4: 在应用启动时扫描目录

**Files:**
- Modify: `src-tauri/src/lib.rs` (setup 函数)

**Step 1: 在 setup 中启动扫描**

```rust
// src-tauri/src/lib.rs setup 函数中

.setup(|app| {
    // ... 现有代码 ...
    
    // 启动时扫描文件目录
    let file_index = app.state::<FileIndexService>();
    tauri::async_runtime::spawn(async move {
        if let Err(e) = file_index.scan_directory().await {
            tracing::error!("Failed to scan directory: {}", e);
        }
    });
    
    Ok(())
})
```

**Step 2: 在 FTP 上传时添加文件到索引**

```rust
// src-tauri/src/ftp/listeners.rs DataEvent::Put 处理中

DataEvent::Put { path, bytes } => {
    // ... 现有代码 ...
    
    // 添加到文件索引
    let full_path = save_path.join(&path);
    if let Some(file_index) = app_handle.try_state::<FileIndexService>() {
        if let Err(e) = file_index.add_file(full_path.clone()).await {
            tracing::warn!("Failed to add file to index: {}", e);
        }
    }
}
```

**Step 3: Commit**

```bash
git add src-tauri/src/lib.rs
git add src-tauri/src/ftp/listeners.rs
git commit -m "feat: scan directory on startup and add files to index on upload"
```

---

## Task 5: 更新前端类型定义

**Files:**
- Modify: `src/types/index.ts`

**Step 1: 添加 FileInfo 类型**

```typescript
// src/types/index.ts

export interface FileInfo {
  path: string;
  filename: string;
  // 注意：后端返回的时间戳需要转换
  // 简化处理，前端只需要 path 和 filename
}
```

**Step 2: Commit**

```bash
git add src/types/index.ts
git commit -m "types: add FileInfo interface"
```

---

## Task 6: 更新 StatsCard 最新照片逻辑

**Files:**
- Modify: `src/components/StatsCard.tsx`
- Modify: `src/stores/serverStore.ts` (可能需要更新 stats 结构)

**Step 1: 修改 StatsCard 显示逻辑**

```typescript
// src/components/StatsCard.tsx

import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';

export const StatsCard = memo(function StatsCard() {
  const { stats } = useServerStore();
  const { config } = useConfigStore();
  
  // 新增：扫描到的最新文件
  const [scannedLatestFile, setScannedLatestFile] = useState<string | null>(null);
  
  useEffect(() => {
    // 组件加载时获取扫描到的最新文件
    const fetchLatestFile = async () => {
      try {
        const latest = await invoke<FileInfo | null>('get_latest_file');
        if (latest) {
          setScannedLatestFile(latest.filename);
        }
      } catch (error) {
        console.error('Failed to get latest file:', error);
      }
    };
    
    fetchLatestFile();
  }, []);

  // 决定显示哪个文件名
  const displayFilename = stats.last_file || scannedLatestFile || '无';

  const handleOpenPreview = useCallback(async () => {
    if (config?.save_path) {
      let targetFile: string | null = null;
      
      if (stats.last_file) {
        // 有上传记录，使用最新上传的文件
        targetFile = stats.last_file;
      } else if (scannedLatestFile) {
        // 无上传记录，使用扫描到的最新文件
        targetFile = scannedLatestFile;
      }
      
      if (targetFile) {
        const fullPath = `${config.save_path}/${targetFile}`;
        await invoke('open_preview_window', { filePath: fullPath });
      }
    }
  }, [stats.last_file, scannedLatestFile, config?.save_path]);

  // ... 渲染部分使用 displayFilename ...
});
```

**Step 2: Commit**

```bash
git add src/components/StatsCard.tsx
git commit -m "feat: update StatsCard to show scanned latest file when no uploads"
```

---

## Task 7: 更新 PreviewWindow 添加导航功能

**Files:**
- Modify: `src/components/PreviewWindow.tsx`

**Step 1: 添加导航状态和方法**

```typescript
// src/components/PreviewWindow.tsx

interface PreviewWindowState {
  isOpen: boolean;
  currentImage: string | null;
  autoBringToFront: boolean;
  currentIndex: number;      // 新增：当前文件索引
  totalFiles: number;        // 新增：文件总数
}

// 在 PreviewWindowContent 中
const [currentIndex, setCurrentIndex] = useState(0);
const [totalFiles, setTotalFiles] = useState(0);

// 加载文件列表和当前索引
useEffect(() => {
  const loadFileInfo = async () => {
    try {
      const files = await invoke<FileInfo[]>('get_file_list');
      setTotalFiles(files.length);
      
      const index = await invoke<number>('get_current_file_index');
      setCurrentIndex(index ?? 0);
    } catch (error) {
      console.error('Failed to load file info:', error);
    }
  };
  
  loadFileInfo();
}, [imagePath]);

// 导航方法
const navigateTo = async (index: number) => {
  if (index < 0 || index >= totalFiles) return;
  
  try {
    const file = await invoke<FileInfo>('navigate_to_file', { index });
    setCurrentIndex(index);
    setState(prev => ({ ...prev, currentImage: file.path }));
    resetZoom();
  } catch (error) {
    console.error('Failed to navigate:', error);
  }
};

const goToPrevious = () => navigateTo(currentIndex + 1); // 更旧
const goToNext = () => navigateTo(currentIndex - 1);     // 更新
const goToOldest = () => navigateTo(totalFiles - 1);     // 最旧
const goToLatest = () => navigateTo(0);                  // 最新
```

**Step 2: 添加键盘事件监听**

```typescript
useEffect(() => {
  const handleKeyDown = async (e: KeyboardEvent) => {
    switch (e.key) {
      case 'ArrowLeft':
        goToPrevious();
        break;
      case 'ArrowRight':
        goToNext();
        break;
      case 'ArrowUp':
        goToPrevious();
        break;
      case 'ArrowDown':
        goToNext();
        break;
      case 'Home':
        goToLatest();
        break;
      case 'End':
        goToOldest();
        break;
      case 'Escape':
        if (isFullscreen) {
          await appWindow.setFullscreen(false);
          await appWindow.setAlwaysOnTop(false);
        }
        break;
    }
  };
  
  window.addEventListener('keydown', handleKeyDown);
  return () => window.removeEventListener('keydown', handleKeyDown);
}, [currentIndex, totalFiles, isFullscreen]);
```

**Step 3: 添加导航按钮**

```tsx
{/* 导航按钮组 */}
<div className="flex items-center gap-1">
  {/* 最旧 */}
  <button
    onClick={goToOldest}
    disabled={currentIndex >= totalFiles - 1}
    className="p-2 text-gray-300 hover:text-white hover:bg-white/10 rounded-lg transition-colors disabled:opacity-30"
    title="最旧 (End)"
  >
    <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M11 19l-7-7 7-7m8 14l-7-7 7-7" />
    </svg>
  </button>
  
  {/* 上一张 */}
  <button
    onClick={goToPrevious}
    disabled={currentIndex >= totalFiles - 1}
    className="p-2 text-gray-300 hover:text-white hover:bg-white/10 rounded-lg transition-colors disabled:opacity-30"
    title="上一张 (← ↑)"
  >
    <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 19l-7-7 7-7" />
    </svg>
  </button>
  
  {/* 下一张 */}
  <button
    onClick={goToNext}
    disabled={currentIndex <= 0}
    className="p-2 text-gray-300 hover:text-white hover:bg-white/10 rounded-lg transition-colors disabled:opacity-30"
    title="下一张 (→ ↓)"
  >
    <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
    </svg>
  </button>
  
  {/* 最新 */}
  <button
    onClick={goToLatest}
    disabled={currentIndex <= 0}
    className="p-2 text-gray-300 hover:text-white hover:bg-white/10 rounded-lg transition-colors disabled:opacity-30"
    title="最新 (Home)"
  >
    <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 5l7 7-7 7M5 5l7 7-7 7" />
    </svg>
  </button>
</div>
```

**Step 4: Commit**

```bash
git add src/components/PreviewWindow.tsx
git commit -m "feat: add image navigation buttons and keyboard shortcuts"
```

---

## Task 8: 更新格式过滤规则

**Files:**
- Modify: `src-tauri/src/ftp/listeners.rs`

**Step 1: 更新 is_supported_image 检查**

```rust
// 在 DataEvent::Put 处理中，更新图片格式检查

let is_image = path.to_lowercase().ends_with(".jpg")
    || path.to_lowercase().ends_with(".jpeg")
    || path.to_lowercase().ends_with(".heif")
    || path.to_lowercase().ends_with(".hif")
    || path.to_lowercase().ends_with(".heic");

if is_image {
    // ... 处理图片
} else {
    // 非图片文件不显示在"最新照片"，不自动打开预览
    info!("Non-image file uploaded, skipping preview: {}", path);
}
```

**Step 2: Commit**

```bash
git add src-tauri/src/ftp/listeners.rs
git commit -m "feat: update image format filter to jpg/jpeg/heif/hif/heic only"
```

---

## Task 9: 构建验证

**Step 1: 构建前端**

Run: `./build.sh frontend`
Expected: 成功编译

**Step 2: 构建 Windows**

Run: `./build.sh windows`
Expected: 成功编译，生成 .exe

**Step 3: 测试功能**
1. 启动应用，检查日志中扫描到的文件数
2. 查看"最新照片"按钮是否显示扫描到的最新文件
3. 上传图片，查看按钮是否切换为最新上传
4. 打开预览窗口，测试导航按钮和快捷键
5. 测试 EXIF 时间读取功能

**Step 4: Commit**

```bash
git add -A
git commit -m "feat: complete image navigation feature with EXIF support"
```

---

## 总结

本实现计划涵盖：
1. ✅ EXIF 读取依赖添加
2. ✅ FileIndexService 后端服务（扫描、排序、EXIF、实时维护）
3. ✅ Tauri IPC 命令
4. ✅ 启动时扫描和上传时更新
5. ✅ StatsCard 最新照片逻辑更新
6. ✅ PreviewWindow 导航按钮和快捷键
7. ✅ 格式过滤规则更新

**预期交付：**
- Windows 平台内置预览支持图片导航
- 支持 jpg/jpeg/heif/hif/heic 格式
- 优先 EXIF 拍摄时间排序
- 快捷键：←↑（旧）→↓（新）Home（最新）End（最旧）
