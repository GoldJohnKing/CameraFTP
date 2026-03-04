use crate::ftp::types::{DomainEvent, ServerStats};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::{trace, warn};
use tauri::Emitter;

/// 事件总线 - 中心化的领域事件分发
#[derive(Debug, Clone)]
pub struct EventBus {
    tx: broadcast::Sender<DomainEvent>,
    last_stats: Arc<RwLock<Option<ServerStats>>>,
}

impl EventBus {
    /// 创建新的事件总线
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(100);
        Self {
            tx,
            last_stats: Arc::new(RwLock::new(None)),
        }
    }

    /// 订阅事件
    pub fn subscribe(&self) -> broadcast::Receiver<DomainEvent> {
        self.tx.subscribe()
    }

    /// 发布事件
    pub fn emit(&self, event: DomainEvent) {
        if let Err(broadcast::error::SendError(_)) = self.tx.send(event) {
            warn!("Event dropped: no active subscribers");
        }
    }

    /// 发布服务器启动事件
    pub fn emit_server_started(&self, bind_addr: impl Into<String>) {
        self.emit(DomainEvent::ServerStarted {
            bind_addr: bind_addr.into(),
        });
    }

    /// 发布服务器停止事件
    pub fn emit_server_stopped(&self) {
        self.emit(DomainEvent::ServerStopped);
    }

    /// 发布文件上传事件
    pub fn emit_file_uploaded(&self, path: impl Into<String>, size: u64) {
        self.emit(DomainEvent::FileUploaded {
            path: path.into(),
            size,
        });
    }

    /// 发布统计更新（带增量检查）
    pub async fn emit_stats_updated(&self, stats: ServerStats) {
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
        loop {
            match self.rx.recv().await {
                Ok(event) => {
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
                    break;
                }
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
        DomainEvent::ServerStarted { .. } => "ServerStarted",
        DomainEvent::ServerStopped { .. } => "ServerStopped",
        DomainEvent::FileUploaded { .. } => "FileUploaded",
        DomainEvent::StatsUpdated { .. } => "StatsUpdated",
    }
}

/// 统计事件处理器 - 将事件转换为前端推送
/// 注意：EventBus.emit_stats_updated() 已做增量检查，这里直接推送即可
pub struct StatsEventHandler {
    app_handle: tauri::AppHandle,
}

impl StatsEventHandler {
    pub fn new(app_handle: tauri::AppHandle) -> Self {
        Self { app_handle }
    }

    /// 向前端发送事件，失败时记录警告日志
    fn emit_to_frontend<T: Serialize + Clone>(&self, event_name: &str, payload: T) {
        if let Err(e) = self.app_handle.emit(event_name, payload) {
            warn!(event = event_name, error = %e, "Failed to emit frontend event");
        }
    }
}

#[async_trait::async_trait]
impl EventHandler for StatsEventHandler {
    async fn handle(&mut self, event: &DomainEvent) {
        match event {
            DomainEvent::StatsUpdated(stats) => {
                // EventBus 已做增量检查，直接推送
                let snapshot = crate::ftp::types::ServerStateSnapshot::from(stats);
                self.emit_to_frontend("stats-update", snapshot);
            }
            DomainEvent::ServerStarted { bind_addr } => {
                // 解析 bind_addr (格式: "ip:port") 以提取 ip 和 port
                let (ip, port) = parse_bind_addr(bind_addr);
                self.emit_to_frontend("server-started", serde_json::json!({
                    "ip": ip,
                    "port": port
                }));
            }
            DomainEvent::ServerStopped { .. } => {
                self.emit_to_frontend("server-stopped", ());
            }
            DomainEvent::FileUploaded { path, size } => {
                self.emit_to_frontend(
                    "file-uploaded",
                    serde_json::json!({ "path": path, "size": size }),
                );
            }
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

/// 解析 bind_addr (格式: "ip:port") 返回 (ip, port)
fn parse_bind_addr(bind_addr: &str) -> (String, u16) {
    let parts: Vec<&str> = bind_addr.split(':').collect();
    if parts.len() == 2 {
        let ip = parts[0].to_string();
        let port = parts[1].parse().unwrap_or(2121);
        (ip, port)
    } else {
        ("0.0.0.0".to_string(), 2121)
    }
}

/// 托盘状态更新处理器 - 监听统计更新并更新托盘图标
/// 替代原有的轮询机制，使用事件驱动更新
pub struct TrayUpdateHandler {
    app_handle: tauri::AppHandle,
    last_client_count: std::sync::atomic::AtomicU32,
}

impl TrayUpdateHandler {
    pub fn new(app_handle: tauri::AppHandle) -> Self {
        Self {
            app_handle,
            last_client_count: std::sync::atomic::AtomicU32::new(0),
        }
    }
}

#[async_trait::async_trait]
impl EventHandler for TrayUpdateHandler {
    async fn handle(&mut self, event: &DomainEvent) {
        match event {
            DomainEvent::StatsUpdated(stats) => {
                let client_count = stats.active_connections as u32;
                let last_count = self.last_client_count.load(std::sync::atomic::Ordering::Relaxed);

                // 仅在客户端数量变化时更新托盘
                if client_count != last_count {
                    crate::platform::get_platform()
                        .update_server_state(&self.app_handle, client_count);
                    self.last_client_count.store(client_count, std::sync::atomic::Ordering::Relaxed);
                }
            }
            DomainEvent::ServerStopped => {
                // 服务器停止时重置计数并更新托盘
                crate::platform::get_platform().update_server_state(&self.app_handle, 0);
                self.last_client_count.store(0, std::sync::atomic::Ordering::Relaxed);
            }
            _ => {}
        }
    }

    fn interested_types(&self) -> Option<Vec<&'static str>> {
        Some(vec!["StatsUpdated", "ServerStopped"])
    }
}
