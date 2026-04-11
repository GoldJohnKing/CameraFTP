// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::sync::{watch, RwLock};
use ts_rs::TS;
use std::sync::Arc;

use crate::config::AuthConfig;

pub fn normalize_ipv4_host(host: &str) -> String {
    host.parse::<std::net::Ipv4Addr>()
        .map(|ip| ip.to_string())
        .unwrap_or_else(|_| "127.0.0.1".to_string())
}

pub fn format_ipv4_socket_addr(host: &str, port: u16) -> String {
    format!("{}:{}", normalize_ipv4_host(host), port)
}

pub fn format_ipv4_ftp_url(host: &str, port: u16) -> String {
    format!("ftp://{}", format_ipv4_socket_addr(host, port))
}

/// FTP 服务器统计数据快照
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ServerStats {
    pub active_connections: u64,
    pub total_uploads: u64,
    pub total_bytes_received: u64,
    pub last_uploaded_file: Option<String>,
}

impl ServerStats {
    pub fn with_connected_clients(mut self, connected_clients: u64) -> Self {
        self.active_connections = connected_clients;
        self
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

impl FtpAuthConfig {
    /// Returns (username, password_info) suitable for display in UI.
    pub fn to_display_credentials(&self) -> (Option<String>, Option<String>) {
        match self {
            Self::Anonymous => (None, None),
            Self::Authenticated { username, .. } => {
                (Some(username.clone()), Some("(配置密码)".to_string()))
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
#[derive(Debug, Clone, PartialEq, serde::Serialize, ts_rs::TS)]
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

impl From<&ServerStateSnapshot> for ServerStats {
    fn from(snapshot: &ServerStateSnapshot) -> Self {
        Self {
            active_connections: snapshot.connected_clients as u64,
            total_uploads: snapshot.files_received,
            total_bytes_received: snapshot.bytes_received,
            last_uploaded_file: snapshot.last_file.clone(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ServerRuntimeSnapshot {
    pub bind_addr: Option<String>,
    pub is_running: bool,
    pub stats: Option<ServerStats>,
}

#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ServerRuntimeView {
    pub server_info: Option<ServerInfo>,
    pub stats: ServerStateSnapshot,
}

#[derive(Debug, Clone)]
pub struct ServerRuntimeState {
    state: Arc<RwLock<ServerRuntimeSnapshot>>,
    tx: watch::Sender<ServerRuntimeSnapshot>,
}

impl Default for ServerRuntimeState {
    fn default() -> Self {
        let snapshot = ServerRuntimeSnapshot::default();
        let (tx, _rx) = watch::channel(snapshot.clone());
        Self {
            state: Arc::new(RwLock::new(snapshot)),
            tx,
        }
    }
}

impl ServerRuntimeState {
    pub fn subscribe(&self) -> watch::Receiver<ServerRuntimeSnapshot> {
        self.tx.subscribe()
    }

    pub async fn update_running_snapshot(&self, snapshot: ServerStateSnapshot) {
        let mut state = self.state.write().await;
        state.is_running = snapshot.is_running;
        if snapshot.is_running {
            state.stats = Some(ServerStats::from(&snapshot));
        } else {
            state.bind_addr = None;
            state.stats = None;
        }
        let _ = self.tx.send(state.clone());
    }

    pub async fn record_server_started(&self, bind_addr: String) {
        let mut state = self.state.write().await;
        state.bind_addr = Some(bind_addr);
        state.is_running = true;
        let _ = self.tx.send(state.clone());
    }

    pub async fn record_stats(&self, stats: ServerStats) {
        let mut state = self.state.write().await;
        if !state.is_running {
            return;
        }
        state.stats = Some(stats);
        let _ = self.tx.send(state.clone());
    }

    pub async fn record_server_stopped(&self) {
        let mut state = self.state.write().await;
        *state = ServerRuntimeSnapshot::default();
        let _ = self.tx.send(state.clone());
    }

    pub async fn current_snapshot(&self) -> ServerStateSnapshot {
        let state = self.current_runtime_snapshot().await;
        if !state.is_running {
            return ServerStateSnapshot::default();
        }
        let stats = state.stats.unwrap_or_default();

        ServerStateSnapshot {
            is_running: state.is_running,
            connected_clients: stats.active_connections as usize,
            files_received: stats.total_uploads,
            bytes_received: stats.total_bytes_received,
            last_file: stats.last_uploaded_file,
        }
    }

    pub async fn current_runtime_snapshot(&self) -> ServerRuntimeSnapshot {
        let state = self.state.read().await;
        state.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        format_ipv4_ftp_url, format_ipv4_socket_addr, normalize_ipv4_host, ServerInfo,
        ServerRuntimeState, ServerStats,
    };

    #[test]
    fn normalize_ipv4_helpers_enforce_ipv4_contract() {
        assert_eq!(normalize_ipv4_host("192.168.1.8"), "192.168.1.8");
        assert_eq!(normalize_ipv4_host("::1"), "127.0.0.1");
        assert_eq!(format_ipv4_socket_addr("::1", 2121), "127.0.0.1:2121");
        assert_eq!(format_ipv4_ftp_url("::1", 2121), "ftp://127.0.0.1:2121");
    }

    #[tokio::test]
    async fn stats_after_stop_do_not_restore_running_state() {
        let runtime_state = ServerRuntimeState::default();

        runtime_state
            .record_server_started("192.168.1.8:2121".to_string())
            .await;
        runtime_state.record_server_stopped().await;
        runtime_state
            .record_stats(ServerStats::default().with_connected_clients(2))
            .await;

        let snapshot = runtime_state.current_snapshot().await;

        assert!(!snapshot.is_running);
        assert_eq!(snapshot.connected_clients, 0);
    }

    #[tokio::test]
    async fn stats_recorded_after_stop_are_ignored_for_runtime_snapshot() {
        let runtime_state = ServerRuntimeState::default();

        runtime_state
            .record_server_started("192.168.1.8:2121".to_string())
            .await;
        runtime_state.record_server_stopped().await;
        runtime_state
            .record_stats(ServerStats {
                active_connections: 3,
                total_uploads: 7,
                total_bytes_received: 1024,
                last_uploaded_file: Some("late.jpg".to_string()),
            })
            .await;

        let snapshot = runtime_state.current_snapshot().await;
        let runtime_snapshot = runtime_state.current_runtime_snapshot().await;

        assert_eq!(snapshot, Default::default());
        assert_eq!(runtime_snapshot, super::ServerRuntimeSnapshot::default());
    }

    #[tokio::test]
    async fn runtime_snapshot_reads_bind_addr_and_stats_atomically() {
        let runtime_state = ServerRuntimeState::default();

        runtime_state
            .record_server_started("192.168.1.8:2121".to_string())
            .await;
        runtime_state
            .record_stats(ServerStats {
                active_connections: 3,
                total_uploads: 7,
                total_bytes_received: 1024,
                last_uploaded_file: Some("latest.jpg".to_string()),
            })
            .await;

        let runtime_snapshot = runtime_state.current_runtime_snapshot().await;

        assert_eq!(
            runtime_snapshot,
            super::ServerRuntimeSnapshot {
                bind_addr: Some("192.168.1.8:2121".to_string()),
                is_running: true,
                stats: Some(ServerStats {
                    active_connections: 3,
                    total_uploads: 7,
                    total_bytes_received: 1024,
                    last_uploaded_file: Some("latest.jpg".to_string()),
                }),
            }
        );
    }

    #[test]
    fn server_info_new_builds_ipv4_ftp_url() {
        let info = ServerInfo::new("192.168.1.8".to_string(), 2121, None, None);

        assert_eq!(info.url, "ftp://192.168.1.8:2121");
    }

    #[test]
    fn ftp_auth_config_to_display_credentials() {
        let anonymous = crate::ftp::types::FtpAuthConfig::Anonymous;
        assert_eq!(anonymous.to_display_credentials(), (None, None));

        let authed = crate::ftp::types::FtpAuthConfig::Authenticated {
            username: "admin".to_string(),
            password_hash: "hash123".to_string(),
        };
        let (user, pass_info) = authed.to_display_credentials();
        assert_eq!(user.as_deref(), Some("admin"));
        assert_eq!(pass_info.as_deref(), Some("(配置密码)"));
    }

    #[test]
    fn future_server_info_contract_falls_back_to_ipv4_loopback_for_ipv6_like_host() {
        let info = ServerInfo::new("::1".to_string(), 2121, None, None);

        assert_eq!(info.ip, "127.0.0.1");
        assert_eq!(info.url, "ftp://127.0.0.1:2121");
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
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "type", content = "data")]
pub enum DomainEvent {
    FileUploaded {
        path: String,
        size: u64,
    },
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
        let ip = normalize_ipv4_host(&ip);
        Self {
            is_running: true,
            ip: ip.clone(),
            port,
            url: format_ipv4_ftp_url(&ip, port),
            username: username.unwrap_or_else(|| "anonymous".to_string()),
            password_info: password_info.unwrap_or_else(|| "(任意密码)".to_string()),
        }
    }
}

#[cfg(test)]
pub mod test_utils {
    use super::DomainEvent;
    use tokio::sync::broadcast;

    /// Test-only event bus that doesn't persist events
    #[derive(Debug, Clone)]
    pub struct TransientEventBus {
        tx: broadcast::Sender<DomainEvent>,
    }

    impl Default for TransientEventBus {
        fn default() -> Self {
            Self::new()
        }
    }

    impl TransientEventBus {
        pub fn new() -> Self {
            let (tx, _) = broadcast::channel(100);
            Self { tx }
        }

        pub fn subscribe(&self) -> broadcast::Receiver<DomainEvent> {
            self.tx.subscribe()
        }

        pub fn emit(&self, event: DomainEvent) {
            let _ = self.tx.send(event);
        }
    }
}
