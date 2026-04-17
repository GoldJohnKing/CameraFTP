// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// AI修图总配置
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", default)]
pub struct AiEditConfig {
    /// 总开关
    pub enabled: bool,
    /// 接收图片后自动触发
    pub auto_edit: bool,
    /// 自动修图提示词
    pub prompt: String,
    /// 手动修图上次使用的提示词
    pub manual_prompt: String,
    /// 手动修图上次使用的模型（空则使用 provider.model）
    pub manual_model: String,
    /// Provider 配置
    pub provider: ProviderConfig,
}

impl Default for AiEditConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            auto_edit: true,
            prompt: String::new(),
            manual_prompt: String::new(),
            manual_model: String::new(),
            provider: ProviderConfig::SeedEdit(SeedEditConfig::default()),
        }
    }
}

/// Provider 配置枚举（预留扩展）
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ProviderConfig {
    SeedEdit(SeedEditConfig),
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self::SeedEdit(SeedEditConfig::default())
    }
}

/// 火山引擎 SeedEdit 配置
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase", default)]
pub struct SeedEditConfig {
    /// API Key
    pub api_key: String,
    /// 模型 ID
    pub model: String,
}

/// Available SeedEdit model options: (display name, API model ID)
pub const SEEDREAM_MODELS: &[(&str, &str)] = &[
    ("Doubao-Seedream-5.0-lite", "doubao-seedream-5-0-260128"),
    ("Doubao-Seedream-4.5", "doubao-seedream-4-5-251128"),
    ("Doubao-Seedream-4.0", "doubao-seedream-4-0-250828"),
];

/// Default model: Doubao-Seedream-5.0-lite
pub const DEFAULT_SEEDREAM_MODEL: &str = "doubao-seedream-5-0-260128";

impl Default for SeedEditConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: DEFAULT_SEEDREAM_MODEL.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_ai_edit_enabled_false() {
        let config = AiEditConfig::default();
        assert!(!config.enabled);
    }

    #[test]
    fn default_config_has_empty_prompt() {
        let config = AiEditConfig::default();
        assert!(config.prompt.is_empty());
    }

    #[test]
    fn default_config_has_seed_edit_provider() {
        let config = AiEditConfig::default();
        assert!(matches!(config.provider, ProviderConfig::SeedEdit(_)));
    }

    #[test]
    fn serde_roundtrip_config() {
        let original = AiEditConfig::default();
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: AiEditConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(original.enabled, deserialized.enabled);
        assert_eq!(original.auto_edit, deserialized.auto_edit);
        assert_eq!(original.prompt, deserialized.prompt);
        assert_eq!(original.manual_prompt, deserialized.manual_prompt);
        assert_eq!(original.manual_model, deserialized.manual_model);
    }

    #[test]
    fn serde_roundtrip_provider_config() {
        let original = ProviderConfig::SeedEdit(SeedEditConfig {
            api_key: "test-key".to_string(),
            model: DEFAULT_SEEDREAM_MODEL.to_string(),
        });
        let json = serde_json::to_string(&original).unwrap();
        assert!(json.contains(r#""type":"seed-edit""#));

        let deserialized: ProviderConfig = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, ProviderConfig::SeedEdit(_)));
    }

    #[test]
    fn serde_seed_edit_config_camel_case() {
        let config = SeedEditConfig {
            api_key: "my-secret-key".to_string(),
            model: DEFAULT_SEEDREAM_MODEL.to_string(),
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains(r#""apiKey""#));
        assert!(!json.contains("api_key"));
        assert!(json.contains(r#""model""#));
    }

    #[test]
    fn config_with_custom_values() {
        let config = AiEditConfig {
            enabled: true,
            auto_edit: false,
            prompt: "enhance colors".to_string(),
            manual_prompt: "manual prompt".to_string(),
            manual_model: "doubao-seedream-4-0-250828".to_string(),
            provider: ProviderConfig::SeedEdit(SeedEditConfig {
                api_key: "sk-test-123".to_string(),
                model: DEFAULT_SEEDREAM_MODEL.to_string(),
            }),
        };
        let json = serde_json::to_string(&config).unwrap();
        let back: AiEditConfig = serde_json::from_str(&json).unwrap();

        assert!(back.enabled);
        assert!(!back.auto_edit);
        assert_eq!(back.prompt, "enhance colors");
        assert_eq!(back.manual_prompt, "manual prompt");
        assert_eq!(back.manual_model, "doubao-seedream-4-0-250828");
        let ProviderConfig::SeedEdit(ref se) = back.provider;
        assert_eq!(se.api_key, "sk-test-123");
    }
}
