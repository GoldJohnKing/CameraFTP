// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::PathBuf;
use std::sync::Arc;

use tauri::{command, State};
use tracing::{debug, error, info};

use crate::error::AppError;
use crate::file_index::FileIndexService;
use crate::file_index::FileInfo;

/// 获取文件列表
#[command]
pub async fn get_file_list(
    file_index: State<'_, FileIndexService>,
) -> Result<Arc<Vec<FileInfo>>, AppError> {
    Ok(file_index.get_files().await)
}

/// 获取当前文件索引
#[command]
pub async fn get_current_file_index(
    file_index: State<'_, FileIndexService>,
) -> Result<Option<usize>, AppError> {
    Ok(file_index.get_current_index().await)
}

/// 导航到指定索引
#[command]
pub async fn navigate_to_file(
    file_index: State<'_, FileIndexService>,
    index: usize,
) -> Result<FileInfo, AppError> {
    file_index.navigate_to(index).await
}

/// 获取最新文件
#[command]
pub async fn get_latest_file(
    file_index: State<'_, FileIndexService>,
) -> Result<Option<FileInfo>, AppError> {
    Ok(file_index.get_latest_file().await)
}

/// 启动文件系统监听（桌面平台）
/// 返回是否成功启动
#[command]
pub async fn start_file_watcher(
    file_index: State<'_, FileIndexService>,
) -> Result<bool, AppError> {
    use std::sync::Arc;

    info!("Starting file watcher...");

    let file_index_arc = Arc::new(file_index.inner().clone());
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
    file_index: State<'_, FileIndexService>,
) -> Result<(), AppError> {
    info!("Stopping file watcher...");
    file_index.stop_watcher().await;
    Ok(())
}

/// 处理文件系统事件（供 Android FileObserver 调用）
/// Android 平台通过 JS Bridge 调用此命令通知 Rust 文件变化
#[command]
pub async fn handle_file_system_event(
    file_index: State<'_, FileIndexService>,
    event_type: String,
    path: String,
) -> Result<(), AppError> {
    debug!(
        "Received file system event from Android: {} - {}",
        event_type, path
    );

    let path_buf = PathBuf::from(&path);

    match event_type.as_str() {
        "created" => {
            info!("Handling external file creation: {}", path);
            file_index.handle_external_created(path_buf).await;
        }
        "deleted" => {
            info!("Handling external file deletion: {}", path);
            file_index.handle_external_deleted(path_buf).await;
        }
        "modified" => {
            // 修改事件通常不需要特殊处理索引
            debug!("File modified (ignoring for index): {}", path);
        }
        _ => {
            debug!("Unknown file event type: {}", event_type);
        }
    }

    Ok(())
}