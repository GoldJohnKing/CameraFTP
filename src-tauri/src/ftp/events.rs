// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::ftp::types::{
    DomainEvent, ServerRuntimeSnapshot, ServerRuntimeState, ServerStateSnapshot, ServerStats,
};
use serde::Serialize;
use tokio::sync::{broadcast, watch};
use tracing::{trace, warn};
use tauri::Emitter;

/// 事件总线 - 中心化的领域事件分发
#[derive(Debug, Clone)]
pub struct EventBus {
    tx: broadcast::Sender<DomainEvent>,
    runtime_state: ServerRuntimeState,
}

impl EventBus {
    /// 创建新的事件总线
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(100);
        Self {
            tx,
            runtime_state: ServerRuntimeState::default(),
        }
    }

    pub fn runtime_state(&self) -> ServerRuntimeState {
        self.runtime_state.clone()
    }

    /// 订阅事件
    pub fn subscribe(&self) -> broadcast::Receiver<DomainEvent> {
        self.tx.subscribe()
    }

    /// 发布服务器启动事件
    pub async fn emit_server_started(&self, bind_addr: impl Into<String>) {
        let bind_addr = bind_addr.into();
        self.runtime_state()
            .record_server_started(bind_addr.clone())
            .await;
        trace!(bind_addr = %bind_addr, "Runtime state recorded for server start");
    }

    /// 发布服务器停止事件
    pub async fn emit_server_stopped(&self) {
        self.runtime_state().record_server_stopped().await;
    }

    /// 发布文件上传事件
    pub fn emit_file_uploaded(&self, path: impl Into<String>, size: u64) {
        let event = DomainEvent::FileUploaded {
            path: path.into(),
            size,
        };
        self.emit_non_state_event(event);
    }

    /// 发布文件索引变化事件
    pub fn emit_file_index_changed(&self, count: usize, latest_filename: Option<String>) {
        let event = DomainEvent::FileIndexChanged {
            count,
            latest_filename,
        };
        self.emit_non_state_event(event);
    }

    /// 发布统计更新
    pub async fn emit_stats_updated(&self, stats: ServerStats) {
        self.runtime_state().record_stats(stats).await;
    }

    fn emit_non_state_event(&self, event: DomainEvent) {
        if let Err(broadcast::error::SendError(_)) = self.tx.send(event) {
            warn!("Event dropped: no active subscribers");
        }
    }

}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

/// 事件处理器trait
#[async_trait::async_trait]
pub trait EventHandler: Send + Sync {
    /// 处理事件
    async fn handle(&mut self, event: &DomainEvent);

    /// 获取感兴趣的事件类型（None表示所有事件）
    fn interested_types(&self) -> Option<Vec<&'static str>> {
        None
    }
}

#[async_trait::async_trait]
pub trait RuntimeStateHandler: Send + Sync {
    async fn handle_runtime_state(&mut self, snapshot: &ServerRuntimeSnapshot);
}

/// 事件处理器管理器
pub struct EventProcessor {
    rx: broadcast::Receiver<DomainEvent>,
    state_rx: watch::Receiver<ServerRuntimeSnapshot>,
    runtime_state_handlers: Vec<Box<dyn RuntimeStateHandler>>,
    event_handlers: Vec<Box<dyn EventHandler>>,
}

impl EventProcessor {
    /// 创建事件处理器
    pub fn new(bus: &EventBus) -> Self {
        let rx = bus.subscribe();
        let state_rx = bus.runtime_state().subscribe();

        Self {
            rx,
            state_rx,
            runtime_state_handlers: Vec::new(),
            event_handlers: Vec::new(),
        }
    }

    pub fn register_runtime_state_handler<H: RuntimeStateHandler + 'static>(
        mut self,
        handler: H,
    ) -> Self {
        self.runtime_state_handlers.push(Box::new(handler));
        self
    }

    /// 注册处理器
    pub fn register<H: EventHandler + 'static>(
        mut self,
        handler: H,
    ) -> Self {
        self.event_handlers.push(Box::new(handler));
        self
    }

    /// 运行处理器循环
    pub async fn run(mut self) {
        self.emit_current_runtime_state().await;
        self.run_loop().await;
    }

    async fn run_loop(&mut self) {
        loop {
            tokio::select! {
                state_changed = self.state_rx.changed() => {
                    match state_changed {
                        Ok(()) => {
                            let snapshot = self.state_rx.borrow_and_update().clone();
                            self.replay_state_to_handlers(&snapshot).await;
                        }
                        Err(_) => break,
                    }
                }
                event_result = self.rx.recv() => {
                    match event_result {
                        Ok(event) => {
                            self.dispatch_event(&event).await;
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            warn!(dropped = n, "Event processor lagged, some events dropped");
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            break;
                        }
                    }
                }
            }
        }
    }

    async fn replay_state_to_handlers(&mut self, runtime_state: &ServerRuntimeSnapshot) {
        for handler in self.runtime_state_handlers.iter_mut() {
            handler.handle_runtime_state(runtime_state).await;
        }
    }

    async fn emit_current_runtime_state(&mut self) {
        let snapshot = self.state_rx.borrow_and_update().clone();
        self.replay_state_to_handlers(&snapshot).await;
    }

    async fn dispatch_event(&mut self, event: &DomainEvent) {
        for handler in self.event_handlers.iter_mut() {
            if should_handle(handler.as_ref(), event) {
                handler.handle(event).await;
            }
        }
    }
}

/// 检查处理器是否应该处理该事件
fn should_handle(handler: &dyn EventHandler, event: &DomainEvent) -> bool {
    match handler.interested_types() {
        None => true,
        Some(types) => types.contains(&event_type_name(event)),
    }
}

/// 获取事件类型名称
fn event_type_name(event: &DomainEvent) -> &'static str {
    match event {
        DomainEvent::FileUploaded { .. } => "FileUploaded",
        DomainEvent::FileIndexChanged { .. } => "FileIndexChanged",
    }
}

/// 统计事件处理器 - 将运行时状态转换为前端推送
pub struct StatsEventHandler {
    app_handle: tauri::AppHandle,
    last_state: Option<RuntimeStateView>,
}

impl StatsEventHandler {
    pub fn new(app_handle: tauri::AppHandle) -> Self {
        Self {
            app_handle,
            last_state: None,
        }
    }

    /// 向前端发送事件，失败时记录警告日志
    fn emit_to_frontend<T: Serialize + Clone>(&self, event_name: &str, payload: T) {
        if let Err(e) = self.app_handle.emit(event_name, payload) {
            warn!(event = event_name, error = %e, "Failed to emit frontend event");
        }
    }

    fn sync_android_service_state(&self, snapshot: &ServerStateSnapshot) {
        crate::platform::get_platform().sync_android_service_state(&self.app_handle, snapshot);
    }

    fn emit_frontend_json(&self, event_name: &str, payload: serde_json::Value) {
        self.emit_to_frontend(event_name, payload);
    }
}

trait ServerEventFanout {
    fn emit_frontend_json(&mut self, event_name: &str, payload: serde_json::Value);
    fn sync_android_service_state(&mut self, snapshot: &ServerStateSnapshot);
}

#[derive(Debug, Clone, PartialEq)]
struct RuntimeStateView {
    bind_addr: Option<String>,
    snapshot: ServerStateSnapshot,
}

impl ServerEventFanout for StatsEventHandler {
    fn emit_frontend_json(&mut self, event_name: &str, payload: serde_json::Value) {
        StatsEventHandler::emit_frontend_json(self, event_name, payload);
    }

    fn sync_android_service_state(&mut self, snapshot: &ServerStateSnapshot) {
        StatsEventHandler::sync_android_service_state(self, snapshot);
    }
}

fn fan_out_runtime_state(
    target: &mut dyn ServerEventFanout,
    previous_state: Option<&RuntimeStateView>,
    runtime_state: &ServerRuntimeSnapshot,
) {
    let snapshot = runtime_state_to_snapshot(runtime_state);
    let current_state = RuntimeStateView {
        bind_addr: runtime_state.bind_addr.clone(),
        snapshot: snapshot.clone(),
    };

    if current_state.snapshot.is_running
        && previous_state.is_none_or(|previous| !previous.snapshot.is_running)
    {
        if let Some(bind_addr) = current_state.bind_addr.as_deref() {
            if let Some((ip, port)) = parse_bind_addr(bind_addr) {
                target.emit_frontend_json(
                    "server-started",
                    serde_json::json!({
                        "ip": ip,
                        "port": port
                    }),
                );
            }
        }
    }

    if current_state.snapshot.is_running {
        let should_emit_stats = previous_state
            .is_none_or(|previous| previous.snapshot != current_state.snapshot);
        if should_emit_stats {
            target.emit_frontend_json(
                "stats-update",
                serde_json::to_value(snapshot.clone())
                    .expect("server snapshot should serialize for frontend events"),
            );
        }
    } else if previous_state.is_some_and(|previous| previous.snapshot.is_running) {
        target.emit_frontend_json("server-stopped", serde_json::Value::Null);
    }

    if previous_state.is_none_or(|previous| previous.snapshot != current_state.snapshot) {
        target.sync_android_service_state(&snapshot);
    }
}

fn runtime_state_to_snapshot(runtime_state: &ServerRuntimeSnapshot) -> ServerStateSnapshot {
    let stats = runtime_state.stats.as_ref();
    ServerStateSnapshot {
        is_running: runtime_state.is_running,
        connected_clients: stats.map_or(0, |stats| stats.active_connections as usize),
        files_received: stats.map_or(0, |stats| stats.total_uploads),
        bytes_received: stats.map_or(0, |stats| stats.total_bytes_received),
        last_file: stats.and_then(|stats| stats.last_uploaded_file.clone()),
    }
}

#[async_trait::async_trait]
impl RuntimeStateHandler for StatsEventHandler {
    async fn handle_runtime_state(&mut self, runtime_state: &ServerRuntimeSnapshot) {
        let previous_state = self.last_state.clone();
        fan_out_runtime_state(self, previous_state.as_ref(), runtime_state);
        self.last_state = Some(RuntimeStateView {
            bind_addr: runtime_state.bind_addr.clone(),
            snapshot: runtime_state_to_snapshot(runtime_state),
        });
    }
}

pub struct FrontendTransientEventHandler {
    app_handle: tauri::AppHandle,
}

impl FrontendTransientEventHandler {
    pub fn new(app_handle: tauri::AppHandle) -> Self {
        Self { app_handle }
    }
}

#[async_trait::async_trait]
impl EventHandler for FrontendTransientEventHandler {
    async fn handle(&mut self, event: &DomainEvent) {
        match event {
            DomainEvent::FileUploaded { path, size } => {
                if let Err(e) = self.app_handle.emit(
                    "file-uploaded",
                    serde_json::json!({ "path": path, "size": size }),
                ) {
                    warn!(event = "file-uploaded", error = %e, "Failed to emit frontend event");
                }
            }
            DomainEvent::FileIndexChanged { count, latest_filename } => {
                if let Err(e) = self.app_handle.emit(
                    "file-index-changed",
                    serde_json::json!({
                        "count": count,
                        "latestFilename": latest_filename
                    }),
                ) {
                    warn!(event = "file-index-changed", error = %e, "Failed to emit frontend event");
                }
            }
        }
    }

    fn interested_types(&self) -> Option<Vec<&'static str>> {
        Some(vec!["FileUploaded", "FileIndexChanged"])
    }
}

/// 解析 bind_addr (格式: "ip:port") 返回 (ip, port)
fn parse_bind_addr(bind_addr: &str) -> Option<(String, u16)> {
    let (host, port) = bind_addr.split_once(':')?;
    if host.contains(':') {
        return None;
    }

    let ip = host.parse::<std::net::Ipv4Addr>().ok()?.to_string();
    let port = port.parse::<u16>().ok()?;

    Some((ip, port))
}

/// 托盘状态更新处理器 - 监听统计更新并更新托盘图标
/// 替代原有的轮询机制，使用事件驱动更新
pub struct TrayUpdateHandler {
    app_handle: tauri::AppHandle,
    last_state: Option<ServerStateSnapshot>,
}

impl TrayUpdateHandler {
    pub fn new(app_handle: tauri::AppHandle) -> Self {
        Self {
            app_handle,
            last_state: None,
        }
    }
}

#[async_trait::async_trait]
impl RuntimeStateHandler for TrayUpdateHandler {
    async fn handle_runtime_state(&mut self, runtime_state: &ServerRuntimeSnapshot) {
        let snapshot = runtime_state_to_snapshot(runtime_state);
        let previous_state = self.last_state.clone();

        if snapshot.is_running && previous_state.as_ref().is_none_or(|previous| !previous.is_running) {
            crate::platform::get_platform().on_server_started(&self.app_handle);
        }

        if !snapshot.is_running && previous_state.as_ref().is_some_and(|previous| previous.is_running) {
            crate::platform::get_platform().on_server_stopped(&self.app_handle);
        } else if snapshot.is_running
            && previous_state.as_ref().is_none_or(|previous| {
                !previous.is_running || previous.connected_clients != snapshot.connected_clients
            })
        {
            crate::platform::get_platform()
                .update_server_state(&self.app_handle, snapshot.connected_clients as u32);
        }

        self.last_state = Some(snapshot);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ftp::types::ServerStats;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    fn parse_bind_addr_future_contract_view(bind_addr: &str) -> Option<(String, u16)> {
        parse_bind_addr(bind_addr)
    }

    #[derive(Clone, Default)]
    struct RecordingHandler {
        events: Arc<Mutex<Vec<DomainEvent>>>,
    }

    #[derive(Clone, Default)]
    struct RecordingRuntimeStateHandler {
        snapshots: Arc<Mutex<Vec<ServerRuntimeSnapshot>>>,
    }

    #[derive(Default)]
    struct RecordingFanout {
        frontend_events: Vec<(String, serde_json::Value)>,
        android_snapshots: Vec<ServerStateSnapshot>,
    }

    impl ServerEventFanout for RecordingFanout {
        fn emit_frontend_json(&mut self, event_name: &str, payload: serde_json::Value) {
            self.frontend_events.push((event_name.to_string(), payload));
        }

        fn sync_android_service_state(&mut self, snapshot: &ServerStateSnapshot) {
            self.android_snapshots.push(snapshot.clone());
        }
    }

    #[async_trait::async_trait]
    impl EventHandler for RecordingHandler {
        async fn handle(&mut self, event: &DomainEvent) {
            self.events.lock().await.push(event.clone());
        }

        fn interested_types(&self) -> Option<Vec<&'static str>> {
            Some(vec![
                "FileUploaded",
                "FileIndexChanged",
            ])
        }
    }

    #[async_trait::async_trait]
    impl RuntimeStateHandler for RecordingRuntimeStateHandler {
        async fn handle_runtime_state(&mut self, snapshot: &ServerRuntimeSnapshot) {
            self.snapshots.lock().await.push(snapshot.clone());
        }
    }

    #[test]
    fn tray_update_handler_calls_platform_update_server_state_for_active_clients() {
        let source = include_str!("events.rs");

        assert!(source.contains("update_server_state(&self.app_handle, snapshot.connected_clients as u32);"));
    }

    #[tokio::test]
    async fn state_event_emitters_maintain_runtime_state_for_consumers() {
        let bus = EventBus::new();

        bus.emit_server_started("127.0.0.1:2121").await;
        bus.emit_stats_updated(ServerStats {
            active_connections: 2,
            total_uploads: 4,
            total_bytes_received: 1024,
            last_uploaded_file: Some("latest.jpg".to_string()),
        })
        .await;

        assert_eq!(
            bus.runtime_state().current_runtime_snapshot().await,
            ServerRuntimeSnapshot {
                bind_addr: Some("127.0.0.1:2121".to_string()),
                is_running: true,
                stats: Some(ServerStats {
                    active_connections: 2,
                    total_uploads: 4,
                    total_bytes_received: 1024,
                    last_uploaded_file: Some("latest.jpg".to_string()),
                }),
            }
        );
    }

    #[tokio::test]
    async fn late_event_processor_replays_running_state_to_handlers() {
        let bus = EventBus::new();
        let runtime_state = bus.runtime_state();
        runtime_state
            .record_server_started("127.0.0.1:2121".to_string())
            .await;
        runtime_state.record_stats(ServerStats {
            active_connections: 2,
            total_uploads: 4,
            total_bytes_received: 1024,
            last_uploaded_file: Some("latest.jpg".to_string()),
        })
        .await;

        let handler = RecordingRuntimeStateHandler::default();
        let snapshots = handler.snapshots.clone();
        let processor = EventProcessor::new(&bus).register_runtime_state_handler(handler);

        let run_handle = tokio::spawn(async move {
            processor.run().await;
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        drop(bus);
        run_handle.await.expect("event processor should exit cleanly");

        let recorded = snapshots.lock().await.clone();
        assert_eq!(
            recorded,
            vec![
                ServerRuntimeSnapshot {
                    bind_addr: Some("127.0.0.1:2121".to_string()),
                    is_running: true,
                    stats: Some(ServerStats {
                        active_connections: 2,
                        total_uploads: 4,
                        total_bytes_received: 1024,
                        last_uploaded_file: Some("latest.jpg".to_string()),
                    }),
                },
            ]
        );
    }

    #[tokio::test]
    async fn late_event_processor_does_not_replay_after_server_stops() {
        let bus = EventBus::new();
        bus.emit_server_started("127.0.0.1:2121").await;
        bus.emit_server_stopped().await;

        let handler = RecordingRuntimeStateHandler::default();
        let snapshots = handler.snapshots.clone();
        let processor = EventProcessor::new(&bus).register_runtime_state_handler(handler);

        let run_handle = tokio::spawn(async move {
            processor.run().await;
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        drop(bus);
        run_handle.await.expect("event processor should exit cleanly");

        assert_eq!(snapshots.lock().await.as_slice(), &[ServerRuntimeSnapshot::default()]);
    }

    #[tokio::test]
    async fn stop_before_run_prevents_stale_replay() {
        let bus = EventBus::new();
        let runtime_state = bus.runtime_state();
        runtime_state
            .record_server_started("127.0.0.1:2121".to_string())
            .await;
        runtime_state.record_stats(ServerStats {
            active_connections: 2,
            total_uploads: 1,
            total_bytes_received: 32,
            last_uploaded_file: Some("before-stop.jpg".to_string()),
        })
        .await;

        let handler = RecordingRuntimeStateHandler::default();
        let snapshots = handler.snapshots.clone();
        let processor = EventProcessor::new(&bus).register_runtime_state_handler(handler);

        runtime_state.record_server_stopped().await;

        let run_handle = tokio::spawn(async move {
            processor.run().await;
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        drop(bus);
        run_handle.await.expect("event processor should exit cleanly");

        assert_eq!(snapshots.lock().await.as_slice(), &[ServerRuntimeSnapshot::default()]);
    }

    #[tokio::test]
    async fn queued_start_then_stop_before_run_does_not_replay_stale_state() {
        let bus = EventBus::new();
        let handler = RecordingRuntimeStateHandler::default();
        let snapshots = handler.snapshots.clone();
        let processor = EventProcessor::new(&bus).register_runtime_state_handler(handler);

        bus.emit_server_started("127.0.0.1:2121").await;
        bus.emit_stats_updated(ServerStats {
            active_connections: 4,
            total_uploads: 2,
            total_bytes_received: 128,
            last_uploaded_file: Some("queued-before-stop.jpg".to_string()),
        })
        .await;
        bus.emit_server_stopped().await;

        let run_handle = tokio::spawn(async move {
            processor.run().await;
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        drop(bus);
        run_handle.await.expect("event processor should exit cleanly");

        assert_eq!(snapshots.lock().await.as_slice(), &[ServerRuntimeSnapshot::default()]);
    }

    #[tokio::test]
    async fn transient_processor_receives_post_subscription_non_state_events() {
        let bus = EventBus::new();
        let handler = RecordingHandler::default();
        let events = handler.events.clone();
        let processor = EventProcessor::new(&bus).register(handler);

        let run_handle = tokio::spawn(async move {
            processor.run().await;
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        bus.emit_server_started("127.0.0.1:2121").await;
        bus.emit_file_uploaded("queued.jpg", 42);
        bus.emit_file_index_changed(3, Some("queued.jpg".to_string()));

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        drop(bus);
        run_handle.await.expect("event processor should exit cleanly");

        assert_eq!(
            events.lock().await.as_slice(),
            &[
                DomainEvent::FileUploaded {
                    path: "queued.jpg".to_string(),
                    size: 42,
                },
                DomainEvent::FileIndexChanged {
                    count: 3,
                    latest_filename: Some("queued.jpg".to_string()),
                },
            ]
        );
    }

    #[tokio::test]
    async fn late_state_subscriber_reads_current_snapshot_without_event_replay() {
        let runtime_state = crate::ftp::types::ServerRuntimeState::default();
        runtime_state.update_running_snapshot(ServerStateSnapshot {
            is_running: true,
            connected_clients: 2,
            files_received: 7,
            bytes_received: 2048,
            last_file: None,
        }).await;

        let snapshot = runtime_state.current_snapshot().await;

        assert!(snapshot.is_running);
        assert_eq!(snapshot.connected_clients, 2);
    }

    #[tokio::test]
    async fn transient_subscriber_does_not_receive_pre_subscription_file_events() {
        let transient_bus = crate::ftp::types::TransientEventBus::new();
        transient_bus.emit(DomainEvent::FileUploaded {
            path: "/before.jpg".into(),
            size: 512,
        });

        let mut rx = transient_bus.subscribe();

        assert!(matches!(
            rx.try_recv(),
            Err(broadcast::error::TryRecvError::Empty)
        ));
    }

    #[tokio::test]
    async fn runtime_state_remains_coherent_across_start_stats_stop() {
        let runtime_state = crate::ftp::types::ServerRuntimeState::default();

        runtime_state
            .record_server_started("192.168.1.8:2121".into())
            .await;
        runtime_state
            .record_stats(ServerStats::default().with_connected_clients(3))
            .await;
        runtime_state.record_server_stopped().await;

        let snapshot = runtime_state.current_snapshot().await;
        assert!(!snapshot.is_running);
        assert_eq!(snapshot.connected_clients, 0);
    }

    #[tokio::test]
    async fn event_emission_does_not_own_runtime_state() {
        let bus = EventBus::new();

        bus.emit_server_started("192.168.1.8:2121").await;
        bus.emit_stats_updated(ServerStats::default().with_connected_clients(3))
            .await;

        let snapshot = bus.runtime_state().current_snapshot().await;

        assert!(snapshot.is_running);
        assert_eq!(snapshot.connected_clients, 3);
    }

    #[tokio::test]
    async fn transient_event_bus_and_event_processor_drop_pre_subscription_events() {
        let bus = EventBus::new();
        bus.emit_file_uploaded("before-processor.jpg", 7);

        let transient_bus = crate::ftp::types::TransientEventBus::new();
        transient_bus.emit(DomainEvent::FileUploaded {
            path: "/before.jpg".into(),
            size: 512,
        });
        let mut transient_rx = transient_bus.subscribe();

        let handler = RecordingHandler::default();
        let events = handler.events.clone();
        let processor = EventProcessor::new(&bus).register(handler);

        let run_handle = tokio::spawn(async move {
            processor.run().await;
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        drop(bus);
        run_handle.await.expect("event processor should exit cleanly");

        assert!(matches!(
            transient_rx.try_recv(),
            Err(broadcast::error::TryRecvError::Empty)
        ));

        assert_eq!(events.lock().await.as_slice(), &[]);
    }

    #[tokio::test]
    async fn non_state_events_emitted_before_processor_creation_are_not_replayed() {
        let bus = EventBus::new();
        bus.emit_file_uploaded("early-upload.jpg", 7);
        bus.emit_file_index_changed(1, Some("early-upload.jpg".to_string()));

        let handler = RecordingHandler::default();
        let events = handler.events.clone();
        let processor = EventProcessor::new(&bus).register(handler);

        let run_handle = tokio::spawn(async move {
            processor.run().await;
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        drop(bus);
        run_handle.await.expect("event processor should exit cleanly");

        assert_eq!(events.lock().await.as_slice(), &[]);
    }

    #[tokio::test]
    async fn runtime_state_replays_started_server_with_latest_stats() {
        let bus = EventBus::new();
        let handler = RecordingRuntimeStateHandler::default();
        let snapshots = handler.snapshots.clone();
        let processor = EventProcessor::new(&bus).register_runtime_state_handler(handler);

        bus.emit_server_started("127.0.0.1:2121").await;
        bus.emit_stats_updated(ServerStats {
            active_connections: 2,
            total_uploads: 5,
            total_bytes_received: 512,
            last_uploaded_file: Some("handoff.jpg".to_string()),
        })
        .await;

        let run_handle = tokio::spawn(async move {
            processor.run().await;
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        drop(bus);
        run_handle.await.expect("event processor should exit cleanly");

        assert_eq!(snapshots.lock().await.as_slice(), &[ServerRuntimeSnapshot {
            bind_addr: Some("127.0.0.1:2121".to_string()),
            is_running: true,
            stats: Some(ServerStats {
                active_connections: 2,
                total_uploads: 5,
                total_bytes_received: 512,
                last_uploaded_file: Some("handoff.jpg".to_string()),
            }),
        }]);
    }

    #[tokio::test]
    async fn stopped_runtime_snapshot_with_stats_does_not_replay_stats_to_late_consumers() {
        let bus = EventBus::new();
        bus.runtime_state()
            .update_running_snapshot(ServerStateSnapshot {
                is_running: false,
                connected_clients: 0,
                files_received: 3,
                bytes_received: 256,
                last_file: Some("stopped.jpg".to_string()),
            })
            .await;

        let handler = RecordingRuntimeStateHandler::default();
        let snapshots = handler.snapshots.clone();
        let processor = EventProcessor::new(&bus).register_runtime_state_handler(handler);

        let run_handle = tokio::spawn(async move {
            processor.run().await;
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        drop(bus);
        run_handle.await.expect("event processor should exit cleanly");

        assert_eq!(
            snapshots.lock().await.as_slice(),
            &[ServerRuntimeSnapshot {
                bind_addr: None,
                is_running: false,
                stats: None,
            }]
        );
    }

    #[tokio::test]
    async fn runtime_state_handlers_update_without_state_domain_event_delivery() {
        let bus = EventBus::new();
        let runtime_handler = RecordingRuntimeStateHandler::default();
        let snapshots = runtime_handler.snapshots.clone();
        let transient_handler = RecordingHandler::default();
        let events = transient_handler.events.clone();
        let processor = EventProcessor::new(&bus)
            .register_runtime_state_handler(runtime_handler)
            .register(transient_handler);

        let run_handle = tokio::spawn(async move {
            processor.run().await;
        });

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        bus.emit_server_started("127.0.0.1:2121").await;
        bus.emit_stats_updated(ServerStats::default().with_connected_clients(2))
            .await;
        bus.emit_file_uploaded("after-start.jpg", 12);

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        drop(bus);
        run_handle.await.expect("event processor should exit cleanly");

        assert!(snapshots.lock().await.iter().any(|snapshot| {
            snapshot.is_running
                && snapshot.stats.as_ref().is_some_and(|stats| stats.active_connections == 2)
        }));
        assert_eq!(
            events.lock().await.as_slice(),
            &[DomainEvent::FileUploaded {
                path: "after-start.jpg".to_string(),
                size: 12,
            }]
        );
    }

    #[tokio::test]
    async fn state_updates_do_not_emit_transient_domain_events() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe();

        bus.emit_server_started("127.0.0.1:2121").await;
        bus.emit_stats_updated(ServerStats::default().with_connected_clients(2))
            .await;
        bus.emit_server_stopped().await;

        assert!(matches!(rx.try_recv(), Err(broadcast::error::TryRecvError::Empty)));
    }

    #[tokio::test]
    async fn stats_updates_after_stop_do_not_restore_runtime_state() {
        let bus = EventBus::new();
        bus.emit_server_started("192.168.1.10:2121").await;
        bus.emit_server_stopped().await;
        bus.emit_stats_updated(ServerStats {
            active_connections: 1,
            total_uploads: 1,
            total_bytes_received: 64,
            last_uploaded_file: Some("ignored-after-stop.jpg".to_string()),
        })
        .await;

        assert_eq!(
            bus.runtime_state().current_runtime_snapshot().await,
            ServerRuntimeSnapshot::default()
        );
    }

    #[tokio::test]
    async fn event_processor_does_not_keep_channel_alive_after_bus_drop() {
        let bus = EventBus::new();
        let processor = EventProcessor::new(&bus);
        drop(bus);

        tokio::time::timeout(std::time::Duration::from_millis(100), processor.run())
            .await
            .expect("event processor should exit after bus drop");
    }

    #[test]
    fn runtime_state_fanout_drives_ui_and_android_sync_from_same_source() {
        let mut fanout = RecordingFanout::default();
        let stats = ServerStats {
            active_connections: 3,
            total_uploads: 7,
            total_bytes_received: 2048,
            last_uploaded_file: Some("dual.jpg".to_string()),
        };

        let started_state = ServerRuntimeSnapshot {
            bind_addr: Some("127.0.0.1:2121".to_string()),
            is_running: true,
            stats: None,
        };
        let running_state = ServerRuntimeSnapshot {
            bind_addr: Some("127.0.0.1:2121".to_string()),
            is_running: true,
            stats: Some(stats.clone()),
        };

        fan_out_runtime_state(
            &mut fanout,
            None,
            &started_state,
        );
        fan_out_runtime_state(
            &mut fanout,
            Some(&RuntimeStateView {
                bind_addr: started_state.bind_addr.clone(),
                snapshot: runtime_state_to_snapshot(&started_state),
            }),
            &running_state,
        );
        fan_out_runtime_state(
            &mut fanout,
            Some(&RuntimeStateView {
                bind_addr: running_state.bind_addr.clone(),
                snapshot: runtime_state_to_snapshot(&running_state),
            }),
            &ServerRuntimeSnapshot::default(),
        );

        assert_eq!(
            fanout.frontend_events,
            vec![
                (
                    "server-started".to_string(),
                    serde_json::json!({ "ip": "127.0.0.1", "port": 2121 }),
                ),
                (
                    "stats-update".to_string(),
                    serde_json::to_value(ServerStateSnapshot::from(&stats))
                        .expect("server snapshot should serialize"),
                ),
                ("server-stopped".to_string(), serde_json::Value::Null),
            ]
        );
        assert_eq!(
            fanout.android_snapshots,
            vec![
                ServerStateSnapshot {
                    is_running: true,
                    ..ServerStateSnapshot::default()
                },
                ServerStateSnapshot::from(&stats),
                ServerStateSnapshot::default(),
            ]
        );
    }

    #[test]
    fn stopped_runtime_snapshot_discards_stats_for_coherent_snapshots() {
        let mut fanout = RecordingFanout::default();
        let started_state = ServerRuntimeSnapshot {
            bind_addr: Some("127.0.0.1:2121".to_string()),
            is_running: true,
            stats: None,
        };
        let stopped_state = ServerRuntimeSnapshot {
            bind_addr: None,
            is_running: false,
            stats: Some(ServerStats {
                active_connections: 0,
                total_uploads: 2,
                total_bytes_received: 128,
                last_uploaded_file: Some("late.jpg".to_string()),
            }),
        };

        fan_out_runtime_state(
            &mut fanout,
            None,
            &started_state,
        );
        fan_out_runtime_state(
            &mut fanout,
            Some(&RuntimeStateView {
                bind_addr: started_state.bind_addr.clone(),
                snapshot: runtime_state_to_snapshot(&started_state),
            }),
            &stopped_state,
        );

        assert_eq!(
            fanout.frontend_events,
            vec![
                (
                    "server-started".to_string(),
                    serde_json::json!({ "ip": "127.0.0.1", "port": 2121 }),
                ),
                (
                    "stats-update".to_string(),
                    serde_json::to_value(ServerStateSnapshot {
                        is_running: true,
                        ..ServerStateSnapshot::default()
                    })
                    .expect("server snapshot should serialize"),
                ),
                ("server-stopped".to_string(), serde_json::Value::Null),
            ]
        );
        assert_eq!(
            fanout.android_snapshots,
            vec![
                ServerStateSnapshot {
                    is_running: true,
                    ..ServerStateSnapshot::default()
                },
                ServerStateSnapshot::default(),
            ]
        );
    }

    #[test]
    fn windows_tray_handler_uses_runtime_state_snapshot_semantics() {
        let source = include_str!("windows.rs");

        assert!(source.contains("fn update_server_state(&self, app: &AppHandle, connected_clients: u32)"));
        assert!(source.contains("TrayIconState::Active"));
        assert!(source.contains("TrayIconState::Idle"));
    }

    #[test]
    fn future_parse_bind_addr_contract_reads_ipv4_host_and_port() {
        assert_eq!(
            parse_bind_addr_future_contract_view("192.168.1.8:2121"),
            Some(("192.168.1.8".to_string(), 2121))
        );
    }

    #[test]
    fn future_parse_bind_addr_contract_rejects_ipv6_like_values() {
        assert_eq!(parse_bind_addr_future_contract_view("::1:2121"), None);
        assert_eq!(parse_bind_addr_future_contract_view("[::1]:2121"), None);
    }

    #[test]
    fn future_parse_bind_addr_contract_rejects_malformed_values() {
        assert_eq!(parse_bind_addr_future_contract_view("192.168.1.8"), None);
        assert_eq!(parse_bind_addr_future_contract_view("192.168.1.8:not-a-port"), None);
        assert_eq!(parse_bind_addr_future_contract_view("not-an-ip:2121"), None);
    }

    #[test]
    fn runtime_state_fanout_skips_server_started_for_invalid_bind_addr() {
        let mut fanout = RecordingFanout::default();
        let started_state = ServerRuntimeSnapshot {
            bind_addr: Some("::1:2121".to_string()),
            is_running: true,
            stats: None,
        };

        fan_out_runtime_state(&mut fanout, None, &started_state);

        assert_eq!(fanout.frontend_events, vec![
            (
                "stats-update".to_string(),
                serde_json::to_value(ServerStateSnapshot {
                    is_running: true,
                    ..ServerStateSnapshot::default()
                })
                .expect("server snapshot should serialize"),
            )
        ]);
    }

}
