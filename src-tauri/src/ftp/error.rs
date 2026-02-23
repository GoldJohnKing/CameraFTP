use std::fmt;

/// FTP模块专用错误类型（仅包含FTP特有错误）
#[derive(Debug)]
pub enum FtpError {
    /// 端口绑定失败
    BindFailed {
        addr: String,
        source: std::io::Error,
    },
    /// 其他IO错误
    Io(std::io::Error),
}

impl fmt::Display for FtpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BindFailed { addr, source } => {
                write!(f, "绑定地址失败 {}: {}", addr, source)
            }
            Self::Io(err) => write!(f, "IO错误: {}", err),
        }
    }
}

impl std::error::Error for FtpError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::BindFailed { source, .. } => Some(source),
            Self::Io(err) => Some(err),
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
