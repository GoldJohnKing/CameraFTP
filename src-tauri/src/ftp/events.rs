use crate::ftp::types::{DomainEvent, ServerStats, StopReason};
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, trace, warn};
use tauri::Emitter;

/// 事件总线配置
#[derive(Debug, Clone)]
pub struct EventBusConfig {
    /// 广播通道容量
    pub broadcast_capacity: usize,
    /// 是否启用增量更新
    pub enable_incremental: bool,
    /// 统计更新防抖间隔（毫秒）
    pub stats_debounce_ms: u64,
}

impl Default for EventBusConfig {
    fn default() -> Self {
        Self {
            broadcast_capacity: 100,
            enable_incremental: true,
            stats_debounce_ms: 500,
        }
    }
}

/// 事件总线 - 中心化的领域事件分发
#[derive(Debug, Clone)]
pub struct EventBus {
    tx: broadcast::Sender<DomainEvent>,
    last_stats: Arc<RwLock<Option<ServerStats>>>,
    config: EventBusConfig,
}

impl EventBus {
    /// 创建新的事件总线
    pub fn new() -> Self {
        Self::with_config(EventBusConfig::default())
    }

    /// 使用配置创建事件总线
    pub fn with_config(config: EventBusConfig) -> Self {
        let (tx, _) = broadcast::channel(config.broadcast_capacity);
        Self {
            tx,
            last_stats: Arc::new(RwLock::new(None)),
            config,
        }
    }

    /// 订阅事件
    pub fn subscribe(&self) -> broadcast::Receiver<DomainEvent> {
        self.tx.subscribe()
    }

    /// 发布事件
    pub fn emit(&self, event: DomainEvent) {
        let _ = self.tx.send(event);
    }

    /// 发布服务器启动事件
    pub fn emit_server_started(&self, bind_addr: impl Into<String>) {
        self.emit(DomainEvent::ServerStarted {
            bind_addr: bind_addr.into(),
        });
    }

    /// 发布服务器停止事件
    pub fn emit_server_stopped(&self, reason: StopReason) {
        self.emit(DomainEvent::ServerStopped { reason });
    }

    /// 发布服务器失败事件
    pub fn emit_server_failed(&self, error: impl Into<String>) {
        self.emit(DomainEvent::ServerFailed {
            error: error.into(),
        });
    }

    /// 发布文件上传事件
    pub fn emit_file_uploaded(&self, path: impl Into<String>, size: u64) {
        self.emit(DomainEvent::FileUploaded {
            path: path.into(),
            size,
        });
    }

    /// 发布会话连接事件
    pub fn emit_session_connected(&self, id: impl Into<String>, username: impl Into<String>) {
        self.emit(DomainEvent::SessionConnected {
            id: id.into(),
            username: username.into(),
        });
    }

    /// 发布会话断开事件
    pub fn emit_session_disconnected(&self, id: impl Into<String>) {
        self.emit(DomainEvent::SessionDisconnected { id: id.into() });
    }

    /// 发布统计更新（带增量检查）
    pub async fn emit_stats_updated(&self, stats: ServerStats) {
        if self.config.enable_incremental {
            let should_emit = {
                let last = self.last_stats.read().await;
                match last.as_ref() {
                    None => true,
                    Some(last_stats) => last_stats.has_changed(&stats),
                }
            };

            if should_emit {
                // 更新最后统计
                let mut last = self.last_stats.write().await;
                *last = Some(stats.clone());
                drop(last);

                // 发布事件
                self.emit(DomainEvent::StatsUpdated(stats));
                trace!("Stats updated event emitted");
            } else {
                trace!("Stats unchanged, skipping event");
            }
        } else {
            self.emit(DomainEvent::StatsUpdated(stats));
        }
    }

    /// 获取订阅者数量
    pub fn subscriber_count(&self) -> usize {
        self.tx.receiver_count()
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

/// 事件处理器管理器
pub struct EventProcessor {
    rx: broadcast::Receiver<DomainEvent>,
    handlers: Vec<Box<dyn EventHandler>>,
}

impl EventProcessor {
    /// 创建事件处理器
    pub fn new(bus: &EventBus) -> Self {
        Self {
            rx: bus.subscribe(),
            handlers: Vec::new(),
        }
    }

    /// 注册处理器
    pub fn register<H: EventHandler + 'static>(
        mut self,
        handler: H,
    ) -> Self {
        self.handlers.push(Box::new(handler));
        self
    }

    /// 运行处理器循环
    pub async fn run(mut self) {
        debug!("EventProcessor started with {} handlers", self.handlers.len());

        loop {
            match self.rx.recv().await {
                Ok(event) => {
                    trace!(event_type = ?std::mem::discriminant(&event), "Processing event");

                    for handler in self.handlers.iter_mut() {
                        if should_handle(handler.as_ref(), &event) {
                            handler.handle(&event).await;
                        }
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    warn!(dropped = n, "Event processor lagged, some events dropped");
                }
                Err(broadcast::error::RecvError::Closed) => {
                    debug!("Event channel closed, stopping processor");
                    break;
                }
            }
        }

        debug!("EventProcessor stopped");
    }
}

/// 检查处理器是否应该处理该事件
fn should_handle(handler: &dyn EventHandler, event: &DomainEvent) -> bool {
    match handler.interested_types() {
        None => true,
        Some(types) => {
            let event_type = event_type_name(event);
            types.contains(&event_type.as_str())
        }
    }
}

/// 获取事件类型名称
fn event_type_name(event: &DomainEvent) -> String {
    match event {
        DomainEvent::ServerStarted { .. } => "ServerStarted".to_string(),
        DomainEvent::ServerStopped { .. } => "ServerStopped".to_string(),
        DomainEvent::ServerFailed { .. } => "ServerFailed".to_string(),
        DomainEvent::FileUploaded { .. } => "FileUploaded".to_string(),
        DomainEvent::FileDownloaded { .. } => "FileDownloaded".to_string(),
        DomainEvent::FileDeleted { .. } => "FileDeleted".to_string(),
        DomainEvent::DirectoryCreated { .. } => "DirectoryCreated".to_string(),
        DomainEvent::DirectoryRemoved { .. } => "DirectoryRemoved".to_string(),
        DomainEvent::FileRenamed { .. } => "FileRenamed".to_string(),
        DomainEvent::SessionConnected { .. } => "SessionConnected".to_string(),
        DomainEvent::SessionDisconnected { .. } => "SessionDisconnected".to_string(),
        DomainEvent::StatsUpdated { .. } => "StatsUpdated".to_string(),
    }
}

/// 统计事件处理器 - 将事件转换为前端推送
pub struct StatsEventHandler {
    app_handle: tauri::AppHandle,
    last_emit: std::time::Instant,
    debounce_duration: std::time::Duration,
}

impl StatsEventHandler {
    pub fn new(app_handle: tauri::AppHandle, debounce_ms: u64) -> Self {
        Self {
            app_handle,
            last_emit: std::time::Instant::now(),
            debounce_duration: std::time::Duration::from_millis(debounce_ms),
        }
    }

    /// 检查是否应该防抖
    fn should_debounce(&mut self) -> bool {
        let now = std::time::Instant::now();
        if now.duration_since(self.last_emit) < self.debounce_duration {
            true
        } else {
            self.last_emit = now;
            false
        }
    }
}

#[async_trait::async_trait]
impl EventHandler for StatsEventHandler {
    async fn handle(&mut self, event: &DomainEvent) {
        match event {
            DomainEvent::StatsUpdated(stats) => {
                if !self.should_debounce() {
                    let snapshot = crate::ftp::types::ServerStateSnapshot::from(stats);
                    let _ = self.app_handle.emit("stats-update", snapshot);
                }
            }
            DomainEvent::ServerStarted { bind_addr } => {
                let _ = self.app_handle.emit("server-started", bind_addr);
            }
            DomainEvent::ServerStopped { .. } => {
                let _ = self.app_handle.emit("server-stopped", ());
            }
            DomainEvent::FileUploaded { path, size } => {
                let _ = self.app_handle.emit(
                    "file-uploaded",
                    serde_json::json!({ "path": path, "size": size }),
                );
            }
            _ => {}
        }
    }

    fn interested_types(&self) -> Option<Vec<&'static str>> {
        Some(vec![
            "StatsUpdated",
            "ServerStarted",
            "ServerStopped",
            "FileUploaded",
        ])
    }
}
