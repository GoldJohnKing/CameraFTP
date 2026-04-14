// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::PathBuf;
use tauri::State;

use crate::ai_edit::AiEditService;
use crate::error::AppError;

#[command]
pub async fn trigger_ai_edit(
    ai_edit: State<'_, AiEditService>,
    file_path: String,
) -> Result<String, AppError> {
    let output_path = ai_edit.edit_single(PathBuf::from(&file_path)).await?;
    Ok(output_path.to_string_lossy().to_string())
}
