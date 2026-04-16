// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_completed_event_serialization_with_output_path() {
        let event = AiEditProgressEvent::Completed {
            current: 1,
            total: 3,
            file_name: "photo.jpg".to_string(),
            failed_count: 0,
            output_path: Some("/output/AIEdit/photo_AIEdit_20260101.jpg".to_string()),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(
            json.contains(r#""type":"completed""#),
            "Expected type=completed, got: {json}"
        );
        assert!(
            json.contains(r#""outputPath":"/output/AIEdit/photo_AIEdit_20260101.jpg""#),
            "Expected outputPath field, got: {json}"
        );
        assert!(json.contains(r#""current":1"#));
        assert!(json.contains(r#""total":3"#));
        assert!(json.contains(r#""fileName":"photo.jpg""#));
        assert!(json.contains(r#""failedCount":0"#));
    }

    #[test]
    fn test_completed_event_serialization_without_output_path() {
        let event = AiEditProgressEvent::Completed {
            current: 2,
            total: 5,
            file_name: "img.png".to_string(),
            failed_count: 1,
            output_path: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(
            json.contains(r#""type":"completed""#),
            "Expected type=completed, got: {json}"
        );
        assert!(
            !json.contains("outputPath"),
            "outputPath should be omitted when None, got: {json}"
        );
    }

    #[test]
    fn test_done_event_serialization_with_output_files() {
        let event = AiEditProgressEvent::Done {
            total: 4,
            failed_count: 1,
            failed_files: vec!["bad.jpg".to_string()],
            output_files: vec![
                "/output/AIEdit/a_AIEdit_20260101.jpg".to_string(),
                "/output/AIEdit/b_AIEdit_20260101.jpg".to_string(),
                "/output/AIEdit/c_AIEdit_20260101.jpg".to_string(),
            ],
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(
            json.contains(r#""type":"done""#),
            "Expected type=done, got: {json}"
        );
        assert!(
            json.contains(r#""outputFiles":["#),
            "Expected outputFiles array, got: {json}"
        );
        assert!(
            json.contains("/output/AIEdit/a_AIEdit_20260101.jpg"),
            "Expected first output file, got: {json}"
        );
        assert!(json.contains(r#""total":4"#));
        assert!(json.contains(r#""failedCount":1"#));
        assert!(json.contains(r#""failedFiles":["bad.jpg"]"#));
    }

    #[test]
    fn test_done_event_deserialization_with_output_files() {
        let json = r#"{"type":"done","total":2,"failedCount":0,"failedFiles":[],"outputFiles":["/out/a.jpg","/out/b.jpg"]}"#;
        let event: AiEditProgressEvent = serde_json::from_str(json).unwrap();
        match event {
            AiEditProgressEvent::Done {
                total,
                failed_count,
                failed_files,
                output_files,
            } => {
                assert_eq!(total, 2);
                assert_eq!(failed_count, 0);
                assert!(failed_files.is_empty());
                assert_eq!(output_files, vec!["/out/a.jpg", "/out/b.jpg"]);
            }
            other => panic!("Expected Done variant, got: {other:?}"),
        }
    }

    #[test]
    fn test_progress_event_serialization_unchanged() {
        let event = AiEditProgressEvent::Progress {
            current: 1,
            total: 10,
            file_name: "test.jpg".to_string(),
            failed_count: 0,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(
            json.contains(r#""type":"progress""#),
            "Expected type=progress, got: {json}"
        );
        assert!(json.contains(r#""current":1"#));
        assert!(json.contains(r#""total":10"#));
        assert!(json.contains(r#""fileName":"test.jpg""#));
        assert!(json.contains(r#""failedCount":0"#));
        assert!(
            !json.contains("outputPath"),
            "Progress variant should not have outputPath"
        );
        assert!(
            !json.contains("outputFiles"),
            "Progress variant should not have outputFiles"
        );
    }

    #[test]
    fn test_failed_event_serialization_unchanged() {
        let event = AiEditProgressEvent::Failed {
            current: 3,
            total: 5,
            file_name: "broken.jpg".to_string(),
            error: "API timeout".to_string(),
            failed_count: 2,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(
            json.contains(r#""type":"failed""#),
            "Expected type=failed, got: {json}"
        );
        assert!(json.contains(r#""error":"API timeout""#));
        assert!(
            !json.contains("outputPath"),
            "Failed variant should not have outputPath"
        );
        assert!(
            !json.contains("outputFiles"),
            "Failed variant should not have outputFiles"
        );
    }

    #[test]
    fn test_queued_event_serialization() {
        let event = AiEditProgressEvent::Queued { queue_depth: 3 };
        let json = serde_json::to_string(&event).unwrap();
        assert!(
            json.contains(r#""type":"queued""#),
            "Expected type=queued, got: {json}"
        );
        assert!(
            json.contains(r#""queueDepth":3"#),
            "Expected queueDepth=3, got: {json}"
        );
    }

    #[test]
    fn test_queued_event_deserialization() {
        let json = r#"{"type":"queued","queueDepth":2}"#;
        let event: AiEditProgressEvent = serde_json::from_str(json).unwrap();
        match event {
            AiEditProgressEvent::Queued { queue_depth } => {
                assert_eq!(queue_depth, 2);
            }
            other => panic!("Expected Queued variant, got: {other:?}"),
        }
    }

    #[test]
    fn test_done_event_deserialization_with_empty_output_files() {
        let json =
            r#"{"type":"done","total":1,"failedCount":1,"failedFiles":["x.jpg"],"outputFiles":[]}"#;
        let event: AiEditProgressEvent = serde_json::from_str(json).unwrap();
        match event {
            AiEditProgressEvent::Done {
                total,
                failed_count,
                failed_files,
                output_files,
            } => {
                assert_eq!(total, 1);
                assert_eq!(failed_count, 1);
                assert_eq!(failed_files, vec!["x.jpg"]);
                assert!(output_files.is_empty());
            }
            other => panic!("Expected Done variant, got: {other:?}"),
        }
    }
}
