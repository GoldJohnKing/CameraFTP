// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::PathBuf;
use tauri::{command, State};

use crate::ai_edit::AiEditService;
use crate::error::AppError;

#[command]
pub async fn trigger_ai_edit(
    ai_edit: State<'_, AiEditService>,
    file_path: String,
    prompt: Option<String>,
    model: Option<String>,
) -> Result<String, AppError> {
    let output_path = ai_edit.edit_single(PathBuf::from(&file_path), prompt, model).await?;
    Ok(output_path.to_string_lossy().to_string())
}

#[command]
pub async fn enqueue_ai_edit(
    ai_edit: State<'_, AiEditService>,
    file_paths: Vec<String>,
    prompt: Option<String>,
    model: Option<String>,
) -> Result<(), AppError> {
    for path in &file_paths {
        ai_edit.enqueue_manual(PathBuf::from(path), prompt.clone(), model.clone()).await?;
    }
    Ok(())
}

#[command]
pub async fn cancel_ai_edit(ai_edit: State<'_, AiEditService>) -> Result<(), AppError> {
    ai_edit.cancel();
    Ok(())
}
