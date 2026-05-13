// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
#[ts(export)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ColorGradingEvent {
    Queued {
        #[serde(rename = "queueDepth")]
        #[ts(rename = "queueDepth")]
        queue_depth: u32,
    },
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
        output_path: String,
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
        #[serde(default)]
        cancelled: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn done_serializes_with_camel_case_keys() {
        let event = ColorGradingEvent::Done {
            total: 2,
            failed_count: 1,
            failed_files: vec!["b.nef".into()],
            output_files: vec!["/out/a_lut.jpg".into()],
            cancelled: false,
        };
        let json = serde_json::to_value(&event).unwrap();
        let obj = json.as_object().unwrap();
        assert_eq!(obj.get("type").unwrap(), "done");
        assert!(obj.contains_key("failedCount"), "expected camelCase 'failedCount', got: {:?}", obj.keys().collect::<Vec<_>>());
        assert!(obj.contains_key("failedFiles"), "expected camelCase 'failedFiles', got: {:?}", obj.keys().collect::<Vec<_>>());
        assert!(obj.contains_key("outputFiles"), "expected camelCase 'outputFiles', got: {:?}", obj.keys().collect::<Vec<_>>());
        assert!(obj.contains_key("cancelled"));
    }

    #[test]
    fn all_variants_roundtrip_through_json() {
        let events: Vec<ColorGradingEvent> = vec![
            ColorGradingEvent::Queued { queue_depth: 2 },
            ColorGradingEvent::Progress {
                current: 1, total: 2, file_name: "a.nef".into(), failed_count: 0,
            },
            ColorGradingEvent::Completed {
                current: 1, total: 2, file_name: "a.nef".into(), failed_count: 0,
                output_path: "/out/a_lut.jpg".into(),
            },
            ColorGradingEvent::Failed {
                current: 2, total: 2, file_name: "b.nef".into(), error: "decode failed".into(),
                failed_count: 1,
            },
            ColorGradingEvent::Done {
                total: 2, failed_count: 1, failed_files: vec!["b.nef".into()],
                output_files: vec!["/out/a_lut.jpg".into()], cancelled: false,
            },
        ];
        for event in &events {
            let json = serde_json::to_string(event).unwrap();
            let back: ColorGradingEvent = serde_json::from_str(&json).unwrap();
            assert_eq!(event, &back, "Roundtrip failed for: {}", json);
        }
    }
}
