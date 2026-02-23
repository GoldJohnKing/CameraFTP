use crate::ftp::types::ServerStats;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, warn};

/// 统计信息Actor命令
#[derive(Debug)]
pub enum StatsCommand {
    GetStats(mpsc::Sender<ServerStats>),
    GetSnapshot(mpsc::Sender<ServerStats>),
    RecordUpload { path: String, bytes: u64 },
    RecordDownload { path: String, bytes: u64 },
    RecordDelete { path: String },
    RecordMkdir { path: String },
    RecordRmdir { path: String },
    RecordRename { from: String, to: String },
    UpdateConnectionCount { count: u64 },
}

/// 统计信息Actor句柄
/// 持有共享状态引用，可以直接读取统计
#[derive(Debug, Clone)]
pub struct StatsActor {
    tx: mpsc::Sender<StatsCommand>,
    /// 共享状态引用，用于直接读取（不经过 channel）
    stats: Arc<RwLock<ServerStats>>,
}

impl StatsActor {
    /// 创建新的统计Actor
    pub fn new() -> (Self, StatsActorWorker) {
        let (tx, rx) = mpsc::channel(100);
        let stats = Arc::new(RwLock::new(ServerStats::default()));
        let worker = StatsActorWorker::new(rx, stats.clone());
        (Self { tx, stats }, worker)
    }

    /// 直接获取当前统计（从共享状态读取，不经过 channel）
    /// 这是更可靠的方式，避免 channel 竞争问题
    pub async fn get_stats_direct(&self) -> ServerStats {
        self.stats.read().await.clone()
    }

    /// 获取当前统计（异步，通过 channel）
    #[deprecated(note = "使用 get_stats_direct() 更可靠")]
    pub async fn get_stats(&self) -> Option<ServerStats> {
        let (tx, mut rx) = mpsc::channel(1);
        if self.tx.send(StatsCommand::GetStats(tx)).await.is_err() {
            warn!("get_stats: channel send failed");
            return None;
        }
        rx.recv().await
    }

    /// 获取统计快照（异步，用于快速读取）
    #[deprecated(note = "使用 get_stats_direct() 更可靠")]
    pub async fn get_snapshot(&self) -> Option<ServerStats> {
        let (tx, mut rx) = mpsc::channel(1);
        if self.tx.send(StatsCommand::GetSnapshot(tx)).await.is_err() {
            warn!("get_snapshot: channel send failed");
            return None;
        }
        rx.recv().await
    }

    /// 记录文件上传
    pub async fn record_upload(&self, path: String, bytes: u64) {
        if let Err(e) = self.tx.send(StatsCommand::RecordUpload { path, bytes }).await {
            warn!("record_upload: channel send failed: {}", e);
        }
    }

    /// 记录文件下载
    pub async fn record_download(&self, path: String, bytes: u64) {
        let _ = self.tx.send(StatsCommand::RecordDownload { path, bytes }).await;
    }

    /// 记录文件删除
    pub async fn record_delete(&self, path: String) {
        let _ = self.tx.send(StatsCommand::RecordDelete { path }).await;
    }

    /// 记录目录创建
    pub async fn record_mkdir(&self, path: String) {
        let _ = self.tx.send(StatsCommand::RecordMkdir { path }).await;
    }

    /// 记录目录删除
    pub async fn record_rmdir(&self, path: String) {
        let _ = self.tx.send(StatsCommand::RecordRmdir { path }).await;
    }

    /// 记录文件重命名
    pub async fn record_rename(&self, from: String, to: String) {
        let _ = self.tx.send(StatsCommand::RecordRename { from, to }).await;
    }

    /// 更新连接数
    pub async fn update_connection_count(&self, count: u64) {
        let _ = self.tx.send(StatsCommand::UpdateConnectionCount { count }).await;
    }
}

/// 统计信息Actor工作者
pub struct StatsActorWorker {
    rx: mpsc::Receiver<StatsCommand>,
    stats: Arc<RwLock<ServerStats>>,
}

impl StatsActorWorker {
    fn new(rx: mpsc::Receiver<StatsCommand>, stats: Arc<RwLock<ServerStats>>) -> Self {
        Self { rx, stats }
    }

    /// 运行Actor主循环
    pub async fn run(mut self) {
        while let Some(cmd) = self.rx.recv().await {
            match cmd {
                StatsCommand::GetStats(tx) => {
                    let stats = self.stats.read().await.clone();
                    let _ = tx.send(stats);
                }
                StatsCommand::GetSnapshot(tx) => {
                    if let Ok(stats) = self.stats.try_read() {
                        let _ = tx.send(stats.clone());
                    } else {
                        let _ = tx.send(ServerStats::default());
                    }
                }
                StatsCommand::RecordUpload { path, bytes } => {
                    let mut stats = self.stats.write().await;
                    stats.total_uploads += 1;
                    stats.total_bytes_received += bytes;
                    stats.last_uploaded_file = Some(path.clone());
                    info!(file = %path, size = bytes, "File uploaded");
                }
                StatsCommand::RecordDownload { path, bytes } => {
                    debug!(file = %path, size = bytes, "File downloaded");
                }
                StatsCommand::RecordDelete { path } => {
                    debug!(file = %path, "File deleted");
                }
                StatsCommand::RecordMkdir { path } => {
                    debug!(dir = %path, "Directory created");
                }
                StatsCommand::RecordRmdir { path } => {
                    debug!(dir = %path, "Directory removed");
                }
                StatsCommand::RecordRename { from, to } => {
                    debug!(from = %from, to = %to, "File renamed");
                }
                StatsCommand::UpdateConnectionCount { count } => {
                    let mut stats = self.stats.write().await;
                    stats.active_connections = count;
                }
            }
        }
    }
}


