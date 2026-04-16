// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

pub(crate) mod seededit;

use async_trait::async_trait;
use crate::error::AppError;
use super::config::ProviderConfig;

#[async_trait]
pub trait AiEditProvider: Send + Sync {
    async fn edit_image(&self, image_base64: &str, mime_type: &str, prompt: &str) -> Result<Vec<u8>, AppError>;
}

pub fn create_provider(config: &ProviderConfig) -> Result<Box<dyn AiEditProvider>, AppError> {
    match config {
        ProviderConfig::SeedEdit(cfg) => {
            Ok(Box::new(seededit::SeedEditProvider::new(cfg)?))
        }
    }
}
