// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::ftp::events::EventBus;
use crate::ftp::types::ServerStats;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::info;

/// 统计信息Actor命令
#[derive(Debug)]
pub enum StatsCommand {
    RecordUpload { path: String, bytes: u64 },
    UpdateConnectionCount { count: u64 },
}

/// 统计信息Actor句柄
/// 持有共享状态引用，用于在测试中直接读取统计（不经过 channel）。
#[derive(Debug, Clone)]
pub struct StatsActor {
    tx: mpsc::Sender<StatsCommand>,
    /// 测试专用：直接读取统计快照（不经过 channel）
    #[cfg(test)]
    stats: Arc<RwLock<ServerStats>>,
}

impl StatsActor {
    /// 创建带 EventBus 的 StatsActor
    /// 当统计信息变化时，通过 EventBus 更新运行时状态（emit_stats_updated）。
    pub fn with_event_bus(event_bus: Option<EventBus>) -> (Self, StatsActorWorker) {
        let (tx, rx) = mpsc::channel(100);
        let stats = Arc::new(RwLock::new(ServerStats::default()));
        let worker = StatsActorWorker::new(rx, stats.clone(), event_bus);
        (Self { tx, #[cfg(test)] stats }, worker)
    }

    /// 直接获取当前统计（从共享状态读取，不经过 channel）
    /// 这是更可靠的方式，避免 channel 竞争问题
    #[cfg(test)]
    pub(crate) async fn get_stats_direct(&self) -> ServerStats {
        self.stats.read().await.clone()
    }

    /// 记录文件上传
    pub async fn record_upload(&self, path: String, bytes: u64) {
        if let Err(e) = self.tx.send(StatsCommand::RecordUpload { path, bytes }).await {
            tracing::warn!("Failed to send record_upload command: {}", e);
        }
    }

    /// 更新连接数
    pub async fn update_connection_count(&self, count: u64) {
        if let Err(e) = self.tx.send(StatsCommand::UpdateConnectionCount { count }).await {
            tracing::warn!("Failed to send update_connection_count command: {}", e);
        }
    }
}

/// 统计信息Actor工作者
pub struct StatsActorWorker {
    rx: mpsc::Receiver<StatsCommand>,
    stats: Arc<RwLock<ServerStats>>,
    /// 事件总线（可选），用于在统计变化时发送事件
    event_bus: Option<EventBus>,
}

impl StatsActorWorker {
    fn new(
        rx: mpsc::Receiver<StatsCommand>,
        stats: Arc<RwLock<ServerStats>>,
        event_bus: Option<EventBus>,
    ) -> Self {
        Self { rx, stats, event_bus }
    }

    /// 运行Actor主循环
    pub async fn run(mut self) {
        while let Some(cmd) = self.rx.recv().await {
            let should_emit = match cmd {
                StatsCommand::RecordUpload { path, bytes } => {
                    let mut stats = self.stats.write().await;
                    stats.total_uploads += 1;
                    stats.total_bytes_received += bytes;
                    stats.last_uploaded_file = Some(path.clone());
                    info!(file = %path, size = bytes, "File uploaded");
                    true // 上传需要发送事件
                }
                StatsCommand::UpdateConnectionCount { count } => {
                    let mut stats = self.stats.write().await;
                    stats.active_connections = count;
                    true // 连接数变化需要发送事件
                }
            };

            // 如果状态有变化且配置了 EventBus，通过 emit_stats_updated 更新运行时状态
            if should_emit {
                if let Some(ref bus) = self.event_bus {
                    let stats = self.stats.read().await.clone();
                    bus.emit_stats_updated(stats).await;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ftp::events::EventBus;
    use std::time::Duration;

    /// 轮询共享统计快照直到条件满足（避免对 channel 时序做假设）
    async fn wait_for_stats(actor: &StatsActor, want: impl Fn(&ServerStats) -> bool) -> ServerStats {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            let stats = actor.get_stats_direct().await;
            if want(&stats) {
                return stats;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "timed out waiting for stats, last seen: {:?}",
                stats
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    #[tokio::test]
    async fn record_upload_accumulates_files_bytes_and_last_file() {
        let (actor, worker) = StatsActor::with_event_bus(None);
        let worker_handle = tokio::spawn(worker.run());

        actor.record_upload("a.jpg".to_string(), 1024).await;
        actor.record_upload("b.nef".to_string(), 2048).await;

        let stats = wait_for_stats(&actor, |s| s.total_uploads == 2).await;
        assert_eq!(stats.total_uploads, 2);
        assert_eq!(stats.total_bytes_received, 1024 + 2048);
        assert_eq!(stats.last_uploaded_file.as_deref(), Some("b.nef"));
        assert_eq!(stats.active_connections, 0, "uploads must not touch connection count");

        worker_handle.abort();
    }

    #[tokio::test]
    async fn update_connection_count_overwrites_previous_value() {
        let (actor, worker) = StatsActor::with_event_bus(None);
        let worker_handle = tokio::spawn(worker.run());

        actor.update_connection_count(3).await;
        let stats = wait_for_stats(&actor, |s| s.active_connections == 3).await;
        assert_eq!(stats.total_uploads, 0, "connection updates must not touch upload counters");

        // 连接数是"设置"语义而非累加：清零覆盖 3
        actor.update_connection_count(0).await;
        let stats = wait_for_stats(&actor, |s| s.active_connections == 0).await;
        assert_eq!(stats.active_connections, 0);

        worker_handle.abort();
    }

    #[tokio::test]
    async fn commands_after_worker_is_dropped_are_discarded_without_panic() {
        // worker 从未运行即被丢弃 → 接收端已关闭；发送方必须静默降级（仅告警），不得 panic
        let (actor, worker) = StatsActor::with_event_bus(None);
        drop(worker);

        actor.record_upload("late.jpg".to_string(), 10).await;
        actor.update_connection_count(1).await;

        assert_eq!(actor.get_stats_direct().await, ServerStats::default());
    }

    #[tokio::test]
    async fn worker_exits_once_all_actor_handles_are_dropped() {
        let (actor, worker) = StatsActor::with_event_bus(None);
        let run = tokio::spawn(worker.run());

        actor.record_upload("x.jpg".to_string(), 1).await;
        wait_for_stats(&actor, |s| s.total_uploads == 1).await;
        drop(actor);

        let exited = tokio::time::timeout(Duration::from_secs(2), run).await;
        assert!(exited.is_ok(), "worker loop must exit after all senders drop");
    }

    #[tokio::test]
    async fn stats_changes_are_forwarded_to_event_bus_runtime_state() {
        let bus = EventBus::new();
        bus.emit_server_started("127.0.0.1:2121").await;

        let (actor, worker) = StatsActor::with_event_bus(Some(bus.clone()));
        let worker_handle = tokio::spawn(worker.run());

        actor.record_upload("via-bus.jpg".to_string(), 512).await;
        actor.update_connection_count(4).await;

        // 等待两条命令都被消费并转发到 EventBus 的运行时状态
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        let snapshot = loop {
            let snapshot = bus.runtime_state().current_runtime_snapshot().await;
            let settled = snapshot
                .stats
                .as_ref()
                .is_some_and(|s| s.active_connections == 4 && s.total_uploads == 1);
            if settled {
                break snapshot;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "stats never reached the EventBus runtime state: {:?}",
                snapshot
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        };

        assert!(snapshot.is_running);
        assert_eq!(
            snapshot.stats,
            Some(ServerStats {
                active_connections: 4,
                total_uploads: 1,
                total_bytes_received: 512,
                last_uploaded_file: Some("via-bus.jpg".to_string()),
            })
        );

        worker_handle.abort();
    }
}
