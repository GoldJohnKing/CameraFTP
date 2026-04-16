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
    /// 预设提示词
    pub prompt: String,
    /// Provider 配置
    pub provider: ProviderConfig,
}

impl Default for AiEditConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            auto_edit: true,
            prompt: String::new(),
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
    /// API Key（用户唯一需要配置的字段）
    pub api_key: String,
}

impl Default for SeedEditConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
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
    }

    #[test]
    fn serde_roundtrip_provider_config() {
        let original = ProviderConfig::SeedEdit(SeedEditConfig {
            api_key: "test-key".to_string(),
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
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains(r#""apiKey""#));
        assert!(!json.contains("api_key"));
    }

    #[test]
    fn config_with_custom_values() {
        let config = AiEditConfig {
            enabled: true,
            auto_edit: false,
            prompt: "enhance colors".to_string(),
            provider: ProviderConfig::SeedEdit(SeedEditConfig {
                api_key: "sk-test-123".to_string(),
            }),
        };
        let json = serde_json::to_string(&config).unwrap();
        let back: AiEditConfig = serde_json::from_str(&json).unwrap();

        assert!(back.enabled);
        assert!(!back.auto_edit);
        assert_eq!(back.prompt, "enhance colors");
        let ProviderConfig::SeedEdit(ref se) = back.provider;
        assert_eq!(se.api_key, "sk-test-123");
    }
}
