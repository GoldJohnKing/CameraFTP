use serde::Serialize;
use thiserror::Error;
use tracing::{error, instrument, warn};

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

    #[error("配置错误: {0}")]
    ConfigError(String),

    #[error("FTP服务器错误: {0}")]
    FtpServerError(String),

    #[error("IO错误: {0}")]
    Io(String),

    #[error("序列化错误: {0}")]
    Serialization(String),

    #[error("网络错误: {0}")]
    NetworkError(String),

    #[error("权限错误: {0}")]
    PermissionError(String),

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
            Self::ConfigError(_) => "CONFIG_ERROR",
            Self::FtpServerError(_) => "FTP_SERVER_ERROR",
            Self::Io(_) => "IO_ERROR",
            Self::Serialization(_) => "SERIALIZATION_ERROR",
            Self::NetworkError(_) => "NETWORK_ERROR",
            Self::PermissionError(_) => "PERMISSION_ERROR",
            Self::Other(_) => "OTHER_ERROR",
        }
    }

    /// 获取用户友好的错误消息
    pub fn user_message(&self,
        lang: Language,
    ) -> String {
        match lang {
            Language::Chinese => self.user_message_cn(),
            Language::English => self.user_message_en(),
        }
    }

    fn user_message_cn(&self) -> String {
        match self {
            Self::ServerAlreadyRunning => {
                "FTP服务器已经在运行中，请先停止当前服务器".to_string()
            }
            Self::ServerNotRunning => {
                "FTP服务器未运行，无法执行此操作".to_string()
            }
            Self::NoAvailablePort => {
                "无法找到可用的端口（1025-65535），请检查系统端口占用情况".to_string()
            }
            Self::NoNetworkInterface => {
                "未检测到可用的网络接口，请检查网络连接".to_string()
            }
            Self::ConfigError(msg) => format!("配置错误: {}", msg),
            Self::FtpServerError(msg) => {
                format!("FTP服务器错误: {}", msg)
            }
            Self::Io(msg) => format!("文件系统错误: {}", msg),
            Self::Serialization(msg) => {
                format!("数据序列化错误: {}", msg)
            }
            Self::NetworkError(msg) => format!("网络错误: {}", msg),
            Self::PermissionError(msg) => {
                format!("权限错误: {}，请检查文件或目录权限", msg)
            }
            Self::Other(msg) => msg.clone(),
        }
    }

    fn user_message_en(&self) -> String {
        match self {
            Self::ServerAlreadyRunning => {
                "FTP server is already running, please stop it first".to_string()
            }
            Self::ServerNotRunning => {
                "FTP server is not running".to_string()
            }
            Self::NoAvailablePort => {
                "No available port found (1025-65535), please check port usage".to_string()
            }
            Self::NoNetworkInterface => {
                "No network interface detected, please check your network connection".to_string()
            }
            Self::ConfigError(msg) => format!("Configuration error: {}", msg),
            Self::FtpServerError(msg) => {
                format!("FTP server error: {}", msg)
            }
            Self::Io(msg) => format!("File system error: {}", msg),
            Self::Serialization(msg) => format!("Serialization error: {}", msg),
            Self::NetworkError(msg) => format!("Network error: {}", msg),
            Self::PermissionError(msg) => {
                format!("Permission error: {}, please check file/directory permissions", msg)
            }
            Self::Other(msg) => msg.clone(),
        }
    }

    /// 判断是否应该重试
    pub fn should_retry(&self,
        attempt: u32,
    ) -> bool {
        match self {
            // 这些错误不应该重试
            Self::ServerAlreadyRunning
            | Self::ServerNotRunning
            | Self::PermissionError(_)
            | Self::ConfigError(_) => false,

            // 这些错误可以有限重试
            Self::NoAvailablePort | Self::NoNetworkInterface | Self::NetworkError(_) => {
                attempt < 3
            }

            // 其他错误尝试一次
            _ => attempt < 1,
        }
    }

    /// 获取建议的重试延迟（毫秒）
    pub fn retry_delay_ms(&self,
        attempt: u32,
    ) -> u64 {
        // 指数退避: 100ms, 200ms, 400ms...
        100 * (2_u64.pow(attempt))
    }

    /// 判断是否是严重错误
    pub fn is_critical(&self) -> bool {
        matches!(
            self,
            Self::PermissionError(_) | Self::ConfigError(_) | Self::FtpServerError(_)
        )
    }

    /// 结构化日志记录
    #[instrument(skip(self), fields(error_code = self.code()))]
    pub fn log(&self,
        context: &str,
    ) {
        let error_info = ErrorInfo {
            code: self.code(),
            message: self.to_string(),
            user_message_cn: self.user_message_cn(),
            user_message_en: self.user_message_en(),
            is_critical: self.is_critical(),
        };

        if self.is_critical() {
            error!(
                context = context,
                error = ?error_info,
                "Critical error occurred"
            );
        } else {
            warn!(
                context = context,
                error = ?error_info,
                "Error occurred"
            );
        }
    }

    /// 记录错误并返回self（用于链式调用）
    #[instrument(skip(self))]
    pub fn logged(self) -> Self {
        self.log("error");
        self
    }
}

/// 错误信息结构（用于日志）
#[derive(Debug, Serialize)]
struct ErrorInfo {
    code: &'static str,
    message: String,
    user_message_cn: String,
    user_message_en: String,
    is_critical: bool,
}

/// 语言枚举
#[derive(Debug, Clone, Copy, Default)]
pub enum Language {
    #[default]
    Chinese,
    English,
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        let msg = err.to_string();

        // 根据错误类型分类
        match err.kind() {
            std::io::ErrorKind::PermissionDenied => {
                AppError::PermissionError(msg)
            }
            std::io::ErrorKind::NotFound => {
                AppError::Io(format!("File not found: {}", msg))
            }
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

impl From<crate::ftp::FtpError> for AppError {
    fn from(err: crate::ftp::FtpError) -> Self {
        match err {
            crate::ftp::FtpError::ServerAlreadyRunning => {
                AppError::ServerAlreadyRunning
            }
            crate::ftp::FtpError::ServerNotRunning => {
                AppError::ServerNotRunning
            }
            crate::ftp::FtpError::BindFailed { addr, source } => {
                AppError::NetworkError(format!("Failed to bind to {}: {}", addr, source))
            }
            crate::ftp::FtpError::InvalidConfiguration(msg) => {
                AppError::ConfigError(msg)
            }
            crate::ftp::FtpError::StorageBackendError(msg) => {
                AppError::Io(msg)
            }
            crate::ftp::FtpError::Io(io_err) => AppError::from(io_err),
            crate::ftp::FtpError::Other(msg) => AppError::Other(msg),
        }
    }
}

impl Serialize for AppError {
    fn serialize<S>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("AppError", 4)?;
        state.serialize_field("code", self.code())?;
        state.serialize_field("message", &self.to_string())?;
        state.serialize_field(
            "userMessage",
            &self.user_message(Language::Chinese),
        )?;
        state.serialize_field("isCritical", &self.is_critical())?;
        state.end()
    }
}

/// 应用结果类型别名
pub type AppResult<T> = Result<T, AppError>;

/// 错误处理辅助函数
pub mod helpers {
    use super::*;

    /// 重试执行异步操作
    pub async fn retry<F, Fut, T>(
        mut operation: F,
        max_attempts: u32,
    ) -> AppResult<T>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = AppResult<T>>,
    {
        let mut last_error = None;

        for attempt in 0..max_attempts {
            match operation().await {
                Ok(result) => return Ok(result),
                Err(err) => {
                    let should_retry = err.should_retry(attempt);
                    last_error = Some(err);

                    if should_retry && attempt < max_attempts - 1 {
                        let delay = last_error
                            .as_ref()
                            .unwrap()
                            .retry_delay_ms(attempt);
                        warn!(
                            attempt = attempt + 1,
                            max_attempts = max_attempts,
                            delay_ms = delay,
                            "Operation failed, retrying..."
                        );
                        tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
                    } else {
                        break;
                    }
                }
            }
        }

        Err(last_error.unwrap())
    }

    /// 带超时的操作
    pub async fn with_timeout<F, Fut, T>(
        operation: F,
        timeout_ms: u64,
    ) -> AppResult<T>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = AppResult<T>>,
    {
        match tokio::time::timeout(
            tokio::time::Duration::from_millis(timeout_ms),
            operation(),
        )
        .await
        {
            Ok(result) => result,
            Err(_) => Err(AppError::Other(format!(
                "Operation timed out after {}ms",
                timeout_ms
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_code() {
        assert_eq!(
            AppError::ServerAlreadyRunning.code(),
            "SERVER_ALREADY_RUNNING"
        );
        assert_eq!(AppError::NoAvailablePort.code(), "NO_AVAILABLE_PORT");
    }

    #[test]
    fn test_user_message() {
        let err = AppError::ServerAlreadyRunning;
        let cn_msg = err.user_message(Language::Chinese);
        let en_msg = err.user_message(Language::English);

        assert!(cn_msg.contains("运行"));
        assert!(en_msg.contains("running"));
    }

    #[test]
    fn test_should_retry() {
        let err = AppError::NoAvailablePort;
        assert!(err.should_retry(0));
        assert!(err.should_retry(1));
        assert!(err.should_retry(2));
        assert!(!err.should_retry(3));

        let err = AppError::ServerAlreadyRunning;
        assert!(!err.should_retry(0));
    }

    #[test]
    fn test_is_critical() {
        assert!(AppError::PermissionError("test".to_string()).is_critical());
        assert!(!AppError::NoAvailablePort.is_critical());
    }

    #[test]
    fn test_error_serialization() {
        let err = AppError::ServerAlreadyRunning;
        let json = serde_json::to_string(&err).unwrap();

        assert!(json.contains("SERVER_ALREADY_RUNNING"));
        assert!(json.contains("code"));
        assert!(json.contains("isCritical"));
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "access denied",
        );
        let app_err: AppError = io_err.into();

        assert!(matches!(app_err, AppError::PermissionError(_)));
    }
}
