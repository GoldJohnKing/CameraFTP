use thiserror::Error;
use serde::Serialize;

/// 应用统一错误类型
#[derive(Error, Debug)]
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
    Io(#[from] std::io::Error),
    
    #[error("序列化错误: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("其他错误: {0}")]
    Other(String),
}

impl From<Box<dyn std::error::Error>> for AppError {
    fn from(err: Box<dyn std::error::Error>) -> Self {
        AppError::Other(err.to_string())
    }
}

impl Serialize for AppError {
    fn serialize<S>(&self,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

/// 结果类型别名
pub type AppResult<T> = Result<T, AppError>;
