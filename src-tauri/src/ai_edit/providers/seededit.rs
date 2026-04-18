// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use crate::error::AppError;
use super::AiEditProvider;
use super::super::config::SeedEditConfig;

const BASE_URL: &str = "https://ark.cn-beijing.volces.com/api/v3";

pub struct SeedEditProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
}

impl SeedEditProvider {
    pub fn new(config: &SeedEditConfig) -> Result<Self, AppError> {
        if config.api_key.is_empty() {
            return Err(AppError::AiEditError("API Key is not configured".to_string()));
        }

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(180))
            .build()
            .map_err(|e| AppError::AiEditError(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            client,
            api_key: config.api_key.clone(),
            model: config.model.clone(),
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
struct SeedEditRequest {
    model: String,
    prompt: String,
    image: String,
    size: &'static str,
    response_format: &'static str,
    watermark: bool,
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

#[derive(Deserialize)]
struct SeedEditErrorResponse {
    error: SeedEditErrorDetail,
}

#[derive(Deserialize)]
struct SeedEditErrorDetail {
    code: Option<String>,
    message: Option<String>,
}

#[async_trait]
impl AiEditProvider for SeedEditProvider {
    async fn edit_image(&self, image_base64: &str, mime_type: &str, prompt: &str) -> Result<Vec<u8>, AppError> {
        let request = SeedEditRequest {
            model: self.model.clone(),
            prompt: prompt.to_string(),
            image: format!("data:{};base64,{}", mime_type, image_base64),
            size: "4K",
            response_format: "url",
            watermark: false,
        };

        let response = self.client
            .post(format!("{}/images/generations", BASE_URL))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| AppError::AiEditError(format!("API request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();

            // Try to parse structured error from API
            if let Ok(err_resp) = serde_json::from_str::<SeedEditErrorResponse>(&body) {
                let code = err_resp.error.code.as_deref().unwrap_or("unknown");
                let message = err_resp.error.message.as_deref().unwrap_or("");
                return Err(AppError::AiEditError(format!(
                    "API error ({}): {} — {}", status, code, message
                )));
            }

            // Fallback: raw body preview
            let body_preview: String = body.chars().take(200).collect();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_new_rejects_empty_api_key() {
        let config = SeedEditConfig::default(); // api_key is empty by default
        let result = SeedEditProvider::new(&config);

        assert!(result.is_err());
        match result {
            Err(AppError::AiEditError(msg)) => assert!(msg.contains("API Key")),
            Err(other) => panic!("Expected AiEditError, got: {:?}", other),
            Ok(_) => panic!("Expected error, got success"),
        }
    }

    #[test]
    fn provider_new_succeeds_with_valid_api_key() {
        let config = SeedEditConfig {
            api_key: "test-api-key-123".to_string(),
            model: "doubao-seedream-4-0-250828".to_string(),
        };
        let result = SeedEditProvider::new(&config);

        assert!(result.is_ok());
    }

    #[test]
    fn default_model_is_5_0_lite() {
        assert_eq!(crate::ai_edit::config::DEFAULT_SEEDREAM_MODEL, "doubao-seedream-5-0-260128");
    }

    #[test]
    fn request_body_serialization() {
        let test_model = "doubao-seedream-4-0-250828";
        let request = SeedEditRequest {
            model: test_model.to_string(),
            prompt: "enhance photo quality".to_string(),
            image: "data:image/jpeg;base64,dGVzdA==".to_string(),
            size: "4K",
            response_format: "url",
            watermark: false,
        };

        let json = serde_json::to_string(&request).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["model"], test_model);
        assert_eq!(parsed["prompt"], "enhance photo quality");
        assert_eq!(parsed["image"], "data:image/jpeg;base64,dGVzdA==");
        assert_eq!(parsed["size"], "4K");
        assert_eq!(parsed["response_format"], "url");
    }
}
