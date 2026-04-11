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
