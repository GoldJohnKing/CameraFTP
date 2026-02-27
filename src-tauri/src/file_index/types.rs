use serde::Serialize;
use std::path::PathBuf;
use std::time::SystemTime;

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

impl Default for FileIndex {
    fn default() -> Self {
        Self::new()
    }
}
