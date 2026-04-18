// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use serde::Serialize;
use thiserror::Error;

/// 应用统一错误类型
#[derive(Error, Debug, Clone)]
pub enum AppError {
    #[error("服务器已在运行")]
    ServerAlreadyRunning,

    #[error("服务器未运行")]
    ServerNotRunning,

    #[error("无可用端口")]
    NoAvailablePort,

    #[error("无可用网络接口")]
    NoNetworkInterface,

    #[error("IO错误: {0}")]
    Io(String),

    #[error("序列化错误: {0}")]
    Serialization(String),

    #[error("网络错误: {0}")]
    NetworkError(String),

    #[error("权限错误: {0}")]
    PermissionError(String),

    #[error("存储权限错误: {0}")]
    StoragePermissionError(String),

    #[error("AI修图错误: {0}")]
    AiEditError(String),

    #[error("其他错误: {0}")]
    Other(String),
}

impl AppError {
    /// 获取错误代码（用于前端识别）
    pub fn code(&self) -> &'static str {
        match self {
            Self::ServerAlreadyRunning => "SERVER_ALREADY_RUNNING",
            Self::ServerNotRunning => "SERVER_NOT_RUNNING",
            Self::NoAvailablePort => "NO_AVAILABLE_PORT",
            Self::NoNetworkInterface => "NO_NETWORK_INTERFACE",
            Self::Io(_) => "IO_ERROR",
            Self::Serialization(_) => "SERIALIZATION_ERROR",
            Self::NetworkError(_) => "NETWORK_ERROR",
            Self::PermissionError(_) => "PERMISSION_ERROR",
            Self::StoragePermissionError(_) => "STORAGE_PERMISSION_ERROR",
            Self::AiEditError(_) => "AI_EDIT_ERROR",
            Self::Other(_) => "OTHER_ERROR",
        }
    }

    /// 获取用户友好的错误消息（中文）
    pub fn user_message(&self) -> String {
        match self {
            Self::ServerAlreadyRunning => "FTP服务器已经在运行中，请先停止当前服务器".to_string(),
            Self::ServerNotRunning => "FTP服务器未运行，无法执行此操作".to_string(),
            Self::NoAvailablePort => {
                "无法找到可用的端口（1025-65535），请检查系统端口占用情况".to_string()
            }
            Self::NoNetworkInterface => "未检测到可用的网络接口，请检查网络连接".to_string(),
            Self::Io(msg) => format!("文件系统错误: {}", msg),
            Self::Serialization(msg) => format!("数据序列化错误: {}", msg),
            Self::NetworkError(msg) => format!("网络错误: {}", msg),
            Self::PermissionError(msg) => format!("权限错误: {}，请检查文件或目录权限", msg),
            Self::StoragePermissionError(msg) => format!("存储权限错误: {}", msg),
            Self::AiEditError(msg) => format!("AI修图错误: {}", msg),
            Self::Other(msg) => msg.clone(),
        }
    }

    /// 判断是否是严重错误
    pub fn is_critical(&self) -> bool {
        matches!(
            self,
            Self::PermissionError(_) | Self::StoragePermissionError(_)
        )
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        let msg = err.to_string();

        match err.kind() {
            std::io::ErrorKind::PermissionDenied => AppError::PermissionError(msg),
            std::io::ErrorKind::NotFound => AppError::Io(format!("File not found: {}", msg)),
            std::io::ErrorKind::AlreadyExists => {
                AppError::Io(format!("File already exists: {}", msg))
            }
            std::io::ErrorKind::AddrInUse => {
                AppError::NetworkError(format!("Address in use: {}", msg))
            }
            _ => AppError::Io(msg),
        }
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::Serialization(err.to_string())
    }
}

impl From<Box<dyn std::error::Error>> for AppError {
    fn from(err: Box<dyn std::error::Error>) -> Self {
        AppError::Other(err.to_string())
    }
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("AppError", 4)?;
        state.serialize_field("code", self.code())?;
        state.serialize_field("message", &self.to_string())?;
        state.serialize_field("userMessage", &self.user_message())?;
        state.serialize_field("isCritical", &self.is_critical())?;
        state.end()
    }
}

/// 应用结果类型别名
pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn io_permission_denied_maps_to_permission_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let app_err: AppError = io_err.into();
        assert!(matches!(app_err, AppError::PermissionError(msg) if msg.contains("access denied")));
    }

    #[test]
    fn io_not_found_maps_to_io_variant() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
        let app_err: AppError = io_err.into();
        assert!(matches!(app_err, AppError::Io(msg) if msg.contains("File not found")));
    }

    #[test]
    fn io_already_exists_maps_to_io_variant() {
        let io_err = std::io::Error::new(std::io::ErrorKind::AlreadyExists, "duplicate");
        let app_err: AppError = io_err.into();
        assert!(matches!(app_err, AppError::Io(msg) if msg.contains("File already exists")));
    }

    #[test]
    fn io_addr_in_use_maps_to_network_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::AddrInUse, "port taken");
        let app_err: AppError = io_err.into();
        assert!(matches!(app_err, AppError::NetworkError(msg) if msg.contains("Address in use")));
    }

    #[test]
    fn io_unexpected_maps_to_generic_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "truncated");
        let app_err: AppError = io_err.into();
        assert!(matches!(app_err, AppError::Io(_)));
    }

    #[test]
    fn serde_json_error_maps_to_serialization() {
        let json_err = serde_json::from_str::<i32>("not a number").unwrap_err();
        let app_err: AppError = json_err.into();
        assert!(matches!(app_err, AppError::Serialization(_)));
    }

    #[test]
    fn code_returns_correct_strings() {
        assert_eq!(AppError::ServerAlreadyRunning.code(), "SERVER_ALREADY_RUNNING");
        assert_eq!(AppError::ServerNotRunning.code(), "SERVER_NOT_RUNNING");
        assert_eq!(AppError::NoAvailablePort.code(), "NO_AVAILABLE_PORT");
        assert_eq!(AppError::NoNetworkInterface.code(), "NO_NETWORK_INTERFACE");
        assert_eq!(AppError::Io("x".into()).code(), "IO_ERROR");
        assert_eq!(AppError::NetworkError("x".into()).code(), "NETWORK_ERROR");
        assert_eq!(AppError::PermissionError("x".into()).code(), "PERMISSION_ERROR");
        assert_eq!(AppError::StoragePermissionError("x".into()).code(), "STORAGE_PERMISSION_ERROR");
        assert_eq!(AppError::AiEditError("x".into()).code(), "AI_EDIT_ERROR");
        assert_eq!(AppError::Other("x".into()).code(), "OTHER_ERROR");
    }

    #[test]
    fn only_permission_errors_are_critical() {
        assert!(AppError::PermissionError("x".into()).is_critical());
        assert!(AppError::StoragePermissionError("x".into()).is_critical());
        assert!(!AppError::ServerAlreadyRunning.is_critical());
        assert!(!AppError::ServerNotRunning.is_critical());
        assert!(!AppError::NoAvailablePort.is_critical());
        assert!(!AppError::Io("x".into()).is_critical());
        assert!(!AppError::NetworkError("x".into()).is_critical());
        assert!(!AppError::AiEditError("x".into()).is_critical());
        assert!(!AppError::Other("x".into()).is_critical());
    }

    #[test]
    fn serialize_produces_expected_fields() {
        let err = AppError::NoAvailablePort;
        let json = serde_json::to_value(&err).expect("serialize");
        assert_eq!(json["code"], "NO_AVAILABLE_PORT");
        assert!(json["userMessage"].is_string());
        assert!(json["message"].is_string());
        assert_eq!(json["isCritical"], false);
    }

    #[test]
    fn serialize_critical_error_has_is_critical_true() {
        let err = AppError::PermissionError("denied".into());
        let json = serde_json::to_value(&err).expect("serialize");
        assert_eq!(json["isCritical"], true);
    }
}
