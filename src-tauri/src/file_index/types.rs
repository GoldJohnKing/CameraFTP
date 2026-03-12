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
