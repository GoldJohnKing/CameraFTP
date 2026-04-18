// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[ts(export)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AiEditProgressEvent {
    Progress {
        current: u32,
        total: u32,
        #[serde(rename = "fileName")]
        #[ts(rename = "fileName")]
        file_name: String,
        #[serde(rename = "failedCount")]
        #[ts(rename = "failedCount")]
        failed_count: u32,
    },
    Completed {
        current: u32,
        total: u32,
        #[serde(rename = "fileName")]
        #[ts(rename = "fileName")]
        file_name: String,
        #[serde(rename = "failedCount")]
        #[ts(rename = "failedCount")]
        failed_count: u32,
        #[serde(rename = "outputPath")]
        #[ts(rename = "outputPath")]
        #[serde(skip_serializing_if = "Option::is_none")]
        output_path: Option<String>,
    },
    Failed {
        current: u32,
        total: u32,
        #[serde(rename = "fileName")]
        #[ts(rename = "fileName")]
        file_name: String,
        error: String,
        #[serde(rename = "failedCount")]
        #[ts(rename = "failedCount")]
        failed_count: u32,
    },
    Queued {
        #[serde(rename = "queueDepth")]
        #[ts(rename = "queueDepth")]
        queue_depth: u32,
    },
    Done {
        total: u32,
        #[serde(rename = "failedCount")]
        #[ts(rename = "failedCount")]
        failed_count: u32,
        #[serde(rename = "failedFiles")]
        #[ts(rename = "failedFiles")]
        failed_files: Vec<String>,
        #[serde(rename = "outputFiles")]
        #[ts(rename = "outputFiles")]
        output_files: Vec<String>,
    },
    QueuedDropped {
        #[serde(rename = "fileName")]
        #[ts(rename = "fileName")]
        file_name: String,
        #[serde(rename = "queueDepth")]
        #[ts(rename = "queueDepth")]
        queue_depth: u32,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_variants_roundtrip_through_json() {
        let events: Vec<AiEditProgressEvent> = vec![
            AiEditProgressEvent::Progress {
                current: 1,
                total: 3,
                file_name: "a.jpg".to_string(),
                failed_count: 0,
            },
            AiEditProgressEvent::Completed {
                current: 1,
                total: 3,
                file_name: "a.jpg".to_string(),
                failed_count: 0,
                output_path: Some("/out/a_AIEdit.jpg".to_string()),
            },
            AiEditProgressEvent::Completed {
                current: 2,
                total: 3,
                file_name: "b.jpg".to_string(),
                failed_count: 1,
                output_path: None,
            },
            AiEditProgressEvent::Failed {
                current: 3,
                total: 3,
                file_name: "c.jpg".to_string(),
                error: "timeout".to_string(),
                failed_count: 1,
            },
            AiEditProgressEvent::Queued { queue_depth: 2 },
            AiEditProgressEvent::QueuedDropped {
                file_name: "photo.jpg".to_string(),
                queue_depth: 32,
            },
            AiEditProgressEvent::Done {
                total: 3,
                failed_count: 1,
                failed_files: vec!["c.jpg".to_string()],
                output_files: vec!["/out/a_AIEdit.jpg".to_string()],
            },
        ];

        for event in &events {
            let json = serde_json::to_string(event).unwrap();
            let back: AiEditProgressEvent = serde_json::from_str(&json).unwrap();
            assert_eq!(event, &back, "Roundtrip failed for variant: json={}", json);
        }
    }

    #[test]
    fn queued_dropped_serializes_with_type_tag() {
        let event = AiEditProgressEvent::QueuedDropped {
            file_name: "photo.jpg".to_string(),
            queue_depth: 32,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"queuedDropped\""), "JSON: {}", json);
    }
}
