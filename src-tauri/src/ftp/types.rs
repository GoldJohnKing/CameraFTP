use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use ts_rs::TS;

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

/// FTP 服务器配置
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServerConfig {
    pub port: u16,
    pub root_path: PathBuf,
    pub passive_port_range: (u16, u16),
    pub idle_timeout_seconds: u64,
}

/// 服务器运行时统计快照
#[derive(Debug, Clone, serde::Serialize, ts_rs::TS)]
#[ts(export)]
pub struct ServerStateSnapshot {
    pub is_running: bool,
    pub connected_clients: usize,
    pub files_received: u64,
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
    ServerStarted { bind_addr: String },
    ServerStopped { reason: StopReason },
    FileUploaded { path: String, size: u64 },
    StatsUpdated(ServerStats),
}

/// 服务器停止原因
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum StopReason {
    UserRequest,
}

/// 服务器连接信息（用于前端显示）
#[derive(Debug, Clone, serde::Serialize, TS)]
#[ts(export)]
pub struct ServerInfo {
    pub is_running: bool,
    pub ip: String,
    pub port: u16,
    pub url: String,
    pub username: String,
    pub password_info: String,
}

impl ServerInfo {
    pub fn new(ip: String, port: u16) -> Self {
        Self {
            is_running: true,
            ip: ip.clone(),
            port,
            url: format!("ftp://{}:{}", ip, port),
            username: "anonymous".to_string(),
            password_info: "(任意密码)".to_string(),
        }
    }
}
