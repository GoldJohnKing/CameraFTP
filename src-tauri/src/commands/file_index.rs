// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::sync::Arc;

use tauri::{command, State};
use tracing::{error, info};

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

/// 启动文件系统监听（桌面平台）
/// 返回是否成功启动
#[command]
pub async fn start_file_watcher(
    file_index: State<'_, Arc<FileIndexService>>,
) -> Result<bool, AppError> {
    info!("Starting file watcher...");

    let file_index_arc = Arc::clone(&file_index);
    match FileIndexService::start_watcher(file_index_arc).await {
        Ok(started) => {
            if started {
                info!("File watcher started successfully");
            } else {
                info!("File watcher not started (may be Android platform)");
            }
            Ok(started)
        }
        Err(e) => {
            error!("Failed to start file watcher: {}", e);
            Err(e)
        }
    }
}

/// 停止文件系统监听
#[command]
pub async fn stop_file_watcher(
    file_index: State<'_, Arc<FileIndexService>>,
) -> Result<(), AppError> {
    info!("Stopping file watcher...");
    file_index.stop_watcher().await;
    Ok(())
}

/// 扫描图库图片（供Android前端调用）
#[command]
pub async fn scan_gallery_images(
    file_index: State<'_, Arc<FileIndexService>>,
) -> Result<Vec<FileInfo>, AppError> {
    file_index.scan_directory().await?;
    let files = file_index.get_files().await;
    Ok(files.to_vec())
}

/// 获取最新图片（供Android前端调用）
#[command]
pub async fn get_latest_image(
    file_index: State<'_, Arc<FileIndexService>>,
) -> Result<Option<FileInfo>, AppError> {
    Ok(file_index.get_latest_file().await)
}
