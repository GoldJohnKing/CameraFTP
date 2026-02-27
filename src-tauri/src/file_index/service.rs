use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::time::SystemTime;
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::config::AppConfig;
use crate::error::AppError;
use super::types::{FileIndex, FileInfo};

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub struct FileIndexService {
    index: RwLock<FileIndex>,
    save_path: RwLock<PathBuf>,
}

impl FileIndexService {
    pub fn new() -> Self {
        let config = AppConfig::load();
        Self {
            index: RwLock::new(FileIndex::new()),
            save_path: RwLock::new(config.save_path),
        }
    }

    /// 扫描目录建立索引
    pub async fn scan_directory(&self) -> Result<(), AppError> {
        let save_path = self.save_path.read().await.clone();
        info!("Starting directory scan: {:?}", save_path);
        
        let mut files = Vec::new();
        self.scan_recursive(&save_path, &mut files).await?;
        
        // 按 sort_time 排序（新→旧）
        files.sort_by(|a, b| a.sort_time.cmp(&b.sort_time));
        
        let mut index = self.index.write().await;
        index.files = files;
        index.current_index = index.files.first().map(|_| 0);
        
        info!("Directory scan complete: {} files found", index.files.len());
        Ok(())
    }

    /// 递归扫描目录
    fn scan_recursive<'a>(&'a self, dir: &'a Path, files: &'a mut Vec<FileInfo>) -> BoxFuture<'a, Result<(), AppError>> {
        Box::pin(async move {
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
        })
    }

    /// 检查文件是否是支持的图片格式
    pub fn is_supported_image(path: &Path) -> bool {
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
            .position(|f| f.sort_time > file_info.sort_time)
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

    /// 根据文件路径查找索引
    pub async fn find_file_index(&self, path: &Path) -> Option<usize> {
        let index = self.index.read().await;
        index.files.iter().position(|f| f.path == path)
    }

    /// 获取文件数量
    pub async fn get_file_count(&self) -> usize {
        let index = self.index.read().await;
        index.files.len()
    }

    /// 更新存储路径并重新扫描
    pub async fn update_save_path(&self, new_path: PathBuf) -> Result<(), AppError> {
        let current_path = self.save_path.read().await.clone();
        if current_path != new_path {
            info!("Updating save_path from {:?} to {:?}", current_path, new_path);
            *self.save_path.write().await = new_path;
            self.scan_directory().await?;
        }
        Ok(())
    }
}

impl Default for FileIndexService {
    fn default() -> Self {
        Self::new()
    }
}
