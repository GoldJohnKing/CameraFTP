use tauri::{command, State};
use std::sync::Arc;

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