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
#[serde(rename_all = "camelCase")]
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
