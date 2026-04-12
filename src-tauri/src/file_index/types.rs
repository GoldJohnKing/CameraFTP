// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

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
    pub sort_time: u64, // 时间戳（毫秒）用于TypeScript
}

use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct FileIndex {
    pub files: Vec<FileInfo>,
    pub files_arc: Arc<Vec<FileInfo>>,
    pub current_index: Option<usize>,
}

impl FileIndex {
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            files_arc: Arc::new(Vec::new()),
            current_index: None,
        }
    }

    /// Refresh `files_arc` to match the current `files` vector.
    /// Call this after any mutation to `files`.
    pub fn refresh_arc(&mut self) {
        self.files_arc = Arc::new(self.files.clone());
    }
}

impl Default for FileIndex {
    fn default() -> Self {
        Self::new()
    }
}
