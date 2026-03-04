use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use ts_rs::TS;

use crate::config::AuthConfig;

/// FTP 服务器统计数据快照
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ServerStats {
    pub active_connections: u64,
    pub total_uploads: u64,
    pub total_bytes_received: u64,
    pub last_uploaded_file: Option<String>,
}

impl ServerStats {
    /// 检查是否与另一个统计对象不同（用于增量更新）
    pub fn has_changed(&self, other: &Self) -> bool {
        self.active_connections != other.active_connections
            || self.total_uploads != other.total_uploads
            || self.total_bytes_received != other.total_bytes_received
            || self.last_uploaded_file != other.last_uploaded_file
    }
}

/// FTP 认证配置 - 使用枚举确保类型安全
/// 两种互斥状态：匿名访问 或 认证访问
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "mode", content = "credentials")]
pub enum FtpAuthConfig {
    /// 允许匿名访问
    Anonymous,
    /// 需要用户名和密码认证
    Authenticated {
        username: String,
        password_hash: String,
    },
}

impl Default for FtpAuthConfig {
    fn default() -> Self {
        Self::Anonymous
    }
}

impl FtpAuthConfig {
    /// 检查是否是匿名访问
    pub fn is_anonymous(&self) -> bool {
        matches!(self, Self::Anonymous)
    }

    /// 获取用户名（如果是认证模式）
    pub fn username(&self) -> Option<&str> {
        match self {
            Self::Anonymous => None,
            Self::Authenticated { username, .. } => Some(username),
        }
    }
}

impl From<&AuthConfig> for FtpAuthConfig {
    fn from(auth: &AuthConfig) -> Self {
        let should_be_anonymous =
            auth.anonymous || auth.username.trim().is_empty() || auth.password_hash.is_empty();

        if should_be_anonymous {
            Self::Anonymous
        } else {
            Self::Authenticated {
                username: auth.username.clone(),
                password_hash: auth.password_hash.clone(),
            }
        }
    }
}

/// FTP 服务器配置
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServerConfig {
    pub port: u16,
    pub root_path: PathBuf,
    pub idle_timeout_seconds: u64,
    pub auth: FtpAuthConfig,
}

/// 服务器运行时统计快照
#[derive(Debug, Clone, serde::Serialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ServerStateSnapshot {
    pub is_running: bool,
    pub connected_clients: usize,
    /// Use number instead of bigint for JSON serialization compatibility
    #[ts(type = "number")]
    pub files_received: u64,
    /// Use number instead of bigint for JSON serialization compatibility
    #[ts(type = "number")]
    pub bytes_received: u64,
    pub last_file: Option<String>,
}

impl Default for ServerStateSnapshot {
    fn default() -> Self {
        Self {
            is_running: false,
            connected_clients: 0,
            files_received: 0,
            bytes_received: 0,
            last_file: None,
        }
    }
}

impl From<&ServerStats> for ServerStateSnapshot {
    fn from(stats: &ServerStats) -> Self {
        Self {
            is_running: true,
            connected_clients: stats.active_connections as usize,
            files_received: stats.total_uploads,
            bytes_received: stats.total_bytes_received,
            last_file: stats.last_uploaded_file.clone(),
        }
    }
}

/// 服务器运行状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum ServerStatus {
    Stopped,
    Starting,
    Running,
    Stopping,
}

impl ServerStatus {
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Running)
    }
}

/// 领域事件 - 用于事件驱动架构
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type", content = "data")]
pub enum DomainEvent {
    ServerStarted {
        bind_addr: String,
    },
    ServerStopped,
    FileUploaded {
        path: String,
        size: u64,
    },
    StatsUpdated(ServerStats),
    /// 文件索引发生变化（添加或删除）
    FileIndexChanged {
        count: usize,
        latest_filename: Option<String>,
    },
}

/// 服务器连接信息（用于前端显示）
#[derive(Debug, Clone, serde::Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    pub is_running: bool,
    pub ip: String,
    pub port: u16,
    pub url: String,
    pub username: String,
    pub password_info: String,
}

impl ServerInfo {
    pub fn new(
        ip: String,
        port: u16,
        username: Option<String>,
        password_info: Option<String>,
    ) -> Self {
        Self {
            is_running: true,
            ip: ip.clone(),
            port,
            url: format!("ftp://{}:{}", ip, port),
            username: username.unwrap_or_else(|| "anonymous".to_string()),
            password_info: password_info.unwrap_or_else(|| "(任意密码)".to_string()),
        }
    }
}
