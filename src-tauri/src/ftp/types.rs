// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{watch, RwLock};
use ts_rs::TS;

use crate::config::AuthConfig;
use crate::ftp::FtpServerHandle;

pub(crate) fn normalize_ipv4_host(host: &str) -> String {
    host.parse::<std::net::Ipv4Addr>()
        .map(|ip| ip.to_string())
        .unwrap_or_else(|_| "127.0.0.1".to_string())
}

pub(crate) fn format_ipv4_socket_addr(host: &str, port: u16) -> String {
    format!("{}:{}", normalize_ipv4_host(host), port)
}

pub(crate) fn format_ipv4_ftp_url(host: &str, port: u16) -> String {
    format!("ftp://{}", format_ipv4_socket_addr(host, port))
}

/// FTP 服务器统计数据快照
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub(crate) struct ServerStats {
    pub active_connections: u64,
    pub total_uploads: u64,
    pub total_bytes_received: u64,
    pub last_uploaded_file: Option<String>,
}

/// FTP 认证配置 - 使用枚举确保类型安全
/// 两种互斥状态：匿名访问 或 认证访问
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "mode", content = "credentials")]
#[derive(Default)]
pub enum FtpAuthConfig {
    /// 允许匿名访问
    #[default]
    Anonymous,
    /// 需要用户名和密码认证
    Authenticated {
        username: String,
        password_hash: String,
    },
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
pub(crate) struct ServerConfig {
    pub port: u16,
    /// 监听 IP；None = 所有接口 (0.0.0.0)，生产默认。
    /// 测试注入回环地址 127.0.0.1：回环监听不触发 Windows 防火墙放通弹窗
    /// （版本号变更会改变测试二进制文件名哈希，导致 0.0.0.0 监听反复弹窗）。
    pub bind_ip: Option<std::net::IpAddr>,
    pub root_path: PathBuf,
    pub idle_timeout_seconds: u64,
    pub auth: FtpAuthConfig,
}

/// 服务器运行时统计快照
#[derive(Debug, Clone, PartialEq, serde::Serialize, ts_rs::TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
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

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ServerRuntimeSnapshot {
    pub bind_addr: Option<String>,
    pub is_running: bool,
    pub(crate) stats: Option<ServerStats>,
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
    pub(crate) fn subscribe(&self) -> watch::Receiver<ServerRuntimeSnapshot> {
        self.tx.subscribe()
    }

    #[cfg(test)]
    pub async fn update_running_snapshot(&self, snapshot: ServerStateSnapshot) {
        let mut state = self.state.write().await;
        state.is_running = snapshot.is_running;
        if snapshot.is_running {
            state.stats = Some(ServerStats {
                active_connections: snapshot.connected_clients as u64,
                total_uploads: snapshot.files_received,
                total_bytes_received: snapshot.bytes_received,
                last_uploaded_file: snapshot.last_file.clone(),
            });
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

    pub(crate) async fn record_stats(&self, stats: ServerStats) {
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

    /// 两个运行时状态是否源自同一实例（共享存储且同一 watch 通道）。
    pub(crate) fn is_same_runtime(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.state, &other.state) && self.tx.same_channel(&other.tx)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        format_ipv4_ftp_url, format_ipv4_socket_addr, normalize_ipv4_host, ServerInfo,
        ServerRuntimeState, ServerStats,
    };

    fn test_stats(active: u64, uploads: u64, bytes: u64, last_file: Option<&str>) -> ServerStats {
        super::test_utils::test_stats(active, uploads, bytes, last_file)
    }

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
        // late-arriving stats after stop should be silently discarded
        runtime_state.record_stats(test_stats(2, 0, 0, None)).await;

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
        // all fields in this late stats update should be ignored
        runtime_state
            .record_stats(test_stats(3, 7, 1024, Some("late.jpg")))
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
        // 3 active connections, 7 completed uploads, 1 KiB received
        runtime_state
            .record_stats(test_stats(3, 7, 1024, Some("latest.jpg")))
            .await;

        let runtime_snapshot = runtime_state.current_runtime_snapshot().await;

        assert_eq!(
            runtime_snapshot,
            super::ServerRuntimeSnapshot {
                bind_addr: Some("192.168.1.8:2121".to_string()),
                is_running: true,
                stats: Some(test_stats(3, 7, 1024, Some("latest.jpg"))),
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

    #[test]
    fn server_handle_identity_follows_actor_runtime_channel() {
        // create_ftp_server 仅构建句柄/Actor 值，不 spawn、不监听，可同步驱动
        let (h1, _actor1, _stats1, _bus1) = crate::ftp::create_ftp_server(None);
        let h1_clone = h1.clone();
        let (h2, _actor2, _stats2, _bus2) = crate::ftp::create_ftp_server(None);

        assert!(
            h1.is_same_actor(&h1_clone),
            "clones of one handle must share the actor identity"
        );
        assert!(
            !h1.is_same_actor(&h2),
            "handles of different actors must not be identified as the same"
        );
    }
}

/// 服务器运行状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub(crate) enum ServerStatus {
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

/// FTP 服务器槽位（`FtpServerState` 的载体，sentinel 状态机）
///
/// 用于序列化并发的启动请求（UI 按钮与托盘菜单同时触发等场景）：
/// - `None`：服务器未运行，可以认领启动权
/// - `Starting`：启动权已被认领，Actor 创建与端口监听进行中；
///   此窗口内其他启动请求一律按“已在运行”拒绝，
///   防止并发启动各自绑定端口、产生无法停止的孤儿服务器
/// - `Running`：服务器运行中，持有可用的服务器句柄
///
/// 状态流转：`None → Starting`（认领）→ `Running`（提交）或回滚为 `None`（失败）。
/// 认领/提交/回滚的时序由 `ftp::server_factory` 保证。
#[derive(Debug, Default)]
pub enum FtpServerSlot {
    #[default]
    None,
    Starting,
    Running(FtpServerHandle),
}

impl FtpServerSlot {
    /// 运行中服务器的句柄（`None`/`Starting` 时返回 `None`）
    pub fn running_handle(&self) -> Option<&FtpServerHandle> {
        match self {
            Self::Running(handle) => Some(handle),
            _ => None,
        }
    }

    /// stop 收尾的清空步骤：仅当槽位当前运行句柄与 `stopped` 指向同一 Actor
    /// 时才复位为 `None`，返回是否实际清空。
    ///
    /// 防止 TOCTOU：stop_server 在锁外 await 停止期间，槽位可能已被并发的
    /// 新启动占据；无条件清空会把新服务器变成无法停止的孤儿监听。
    pub(crate) fn clear_if_same_actor(&mut self, stopped: &FtpServerHandle) -> bool {
        let is_stopped_server = self
            .running_handle()
            .is_some_and(|handle| handle.is_same_actor(stopped));
        if is_stopped_server {
            *self = Self::None;
            true
        } else {
            false
        }
    }
}

/// `FtpServerHandle` 的身份判定（inherent impl 置于 types.rs，
/// 与 `FtpServerSlot` 的状态机清空逻辑同处一地，便于对照维护）
impl FtpServerHandle {
    /// 两个句柄是否指向同一 Actor（同一运行时通道）。
    ///
    /// 每次服务器启动（`create_ftp_server`）都会创建全新 `EventBus`，
    /// 句柄持有的 runtime watch 通道与 Actor 的命令通道一一对应：
    /// 同一 Actor 的克隆共享该通道，不同 Actor 的句柄必然不同。
    /// 用于 stop 收尾时判断槽位是否仍由刚停止的那台服务器占据。
    pub fn is_same_actor(&self, other: &Self) -> bool {
        self.runtime_state().is_same_runtime(&other.runtime_state())
    }
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
pub(crate) mod test_utils {
    use super::ServerStats;

    pub(crate) fn test_stats(
        active: u64,
        uploads: u64,
        bytes: u64,
        last_file: Option<&str>,
    ) -> ServerStats {
        ServerStats {
            active_connections: active,
            total_uploads: uploads,
            total_bytes_received: bytes,
            last_uploaded_file: last_file.map(String::from),
        }
    }
}
