use std::fmt;

/// FTP模块专用错误类型
#[derive(Debug)]
pub enum FtpError {
    ServerAlreadyRunning,
    ServerNotRunning,
    BindFailed { addr: String, source: std::io::Error },
    InvalidConfiguration(String),
    StorageBackendError(String),
    Io(std::io::Error),
    Other(String),
}

impl fmt::Display for FtpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ServerAlreadyRunning => write!(f, "FTP服务器已在运行"),
            Self::ServerNotRunning => write!(f, "FTP服务器未运行"),
            Self::BindFailed { addr, source } => {
                write!(f, "绑定地址失败 {}: {}", addr, source)
            }
            Self::InvalidConfiguration(msg) => write!(f, "配置错误: {}", msg),
            Self::StorageBackendError(msg) => write!(f, "存储后端错误: {}", msg),
            Self::Io(err) => write!(f, "IO错误: {}", err),
            Self::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for FtpError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::BindFailed { source, .. } => Some(source),
            Self::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for FtpError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

/// FTP操作结果类型
pub type FtpResult<T> = Result<T, FtpError>;
