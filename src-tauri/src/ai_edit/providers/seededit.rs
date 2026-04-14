// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use crate::error::AppError;
use super::AiEditProvider;
use super::super::config::SeedEditConfig;

const BASE_URL: &str = "https://ark.cn-beijing.volces.com/api/v3";
const MODEL: &str = "doubao-seededit-3-0-i2i-250628";

pub struct SeedEditProvider {
    client: reqwest::Client,
    api_key: String,
}

impl SeedEditProvider {
    pub fn new(config: &SeedEditConfig) -> Result<Self, AppError> {
        if config.api_key.is_empty() {
            return Err(AppError::AiEditError("API Key is not configured".to_string()));
        }

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .map_err(|e| AppError::AiEditError(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            client,
            api_key: config.api_key.clone(),
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
struct SeedEditRequest {
    model: &'static str,
    prompt: String,
    image: String,
    response_format: &'static str,
}

#[derive(Deserialize)]
struct SeedEditResponse {
    data: Vec<SeedEditImageData>,
}

#[derive(Deserialize)]
struct SeedEditImageData {
    url: Option<String>,
    b64_json: Option<String>,
}

#[async_trait]
impl AiEditProvider for SeedEditProvider {
    async fn edit_image(&self, image_base64: &str, mime_type: &str, prompt: &str) -> Result<Vec<u8>, AppError> {
        let request = SeedEditRequest {
            model: MODEL,
            prompt: prompt.to_string(),
            image: format!("data:{};base64,{}", mime_type, image_base64),
            response_format: "url",
        };

        let response = self.client
            .post(format!("{}/v1/images/generations", BASE_URL))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| AppError::AiEditError(format!("API request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            let body_preview = &body[..body.len().min(200)];
            return Err(AppError::AiEditError(format!(
                "API returned {}: {}", status, body_preview
            )));
        }

        let parsed: SeedEditResponse = response
            .json()
            .await
            .map_err(|e| AppError::AiEditError(format!("Failed to parse API response: {}", e)))?;

        let image_data = parsed.data.into_iter().next()
            .ok_or_else(|| AppError::AiEditError("API returned no image data".to_string()))?;

        if let Some(url) = image_data.url {
            let image_bytes = self.client
                .get(&url)
                .send()
                .await
                .map_err(|e| AppError::AiEditError(format!("Failed to download edited image: {}", e)))?
                .bytes()
                .await
                .map_err(|e| AppError::AiEditError(format!("Failed to read image bytes: {}", e)))?;

            Ok(image_bytes.to_vec())
        } else if let Some(b64) = image_data.b64_json {
            use base64::Engine;
            base64::engine::general_purpose::STANDARD.decode(&b64)
                .map_err(|e| AppError::AiEditError(format!("Failed to decode base64 image: {}", e)))
        } else {
            Err(AppError::AiEditError("API returned neither URL nor base64 data".to_string()))
        }
    }
}
