// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::Arc;

use tauri::{command, State};

use crate::error::AppError;
use crate::file_index::FileIndexService;
use crate::file_index::FileInfo;

/// 获取文件列表
#[command]
pub async fn get_file_list(
    file_index: State<'_, Arc<FileIndexService>>,
) -> Result<Arc<Vec<FileInfo>>, AppError> {
    Ok(file_index.get_files().await)
}

/// 获取当前文件索引
#[command]
pub async fn get_current_file_index(
    file_index: State<'_, Arc<FileIndexService>>,
) -> Result<Option<usize>, AppError> {
    Ok(file_index.get_current_index().await)
}

/// 导航到指定索引
#[command]
pub async fn navigate_to_file(
    file_index: State<'_, Arc<FileIndexService>>,
    index: usize,
) -> Result<FileInfo, AppError> {
    file_index.navigate_to(index).await
}

/// 获取最新文件
#[command]
pub async fn get_latest_file(
    file_index: State<'_, Arc<FileIndexService>>,
) -> Result<Option<FileInfo>, AppError> {
    Ok(file_index.get_latest_file().await)
}

/// 获取最新图片（供Android前端调用）
#[command]
pub async fn get_latest_image(
    file_index: State<'_, Arc<FileIndexService>>,
) -> Result<Option<FileInfo>, AppError> {
    Ok(file_index.get_latest_file().await)
}
