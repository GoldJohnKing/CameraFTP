use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::SystemTime;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct FileInfo {
    pub path: PathBuf,
    pub filename: String,
    #[ts(skip)]
    pub exif_time: Option<SystemTime>,
    #[ts(skip)]
    pub modified_time: SystemTime,
    #[ts(skip)]
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
