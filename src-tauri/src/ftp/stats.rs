use crate::ftp::types::{ServerStats, TransferDirection};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, trace};

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
#[derive(Debug, Clone)]
pub struct StatsActor {
    tx: mpsc::Sender<StatsCommand>,
}

impl StatsActor {
    /// 创建新的统计Actor
    pub fn new() -> (Self, StatsActorWorker) {
        let (tx, rx) = mpsc::channel(100);
        let worker = StatsActorWorker::new(rx);
        (Self { tx }, worker)
    }

    /// 获取当前统计（异步）
    pub async fn get_stats(&self) -> Option<ServerStats> {
        let (tx, mut rx) = mpsc::channel(1);
        if self.tx.send(StatsCommand::GetStats(tx)).await.is_err() {
            return None;
        }
        rx.recv().await
    }

    /// 获取统计快照（异步，用于快速读取）
    pub async fn get_snapshot(&self) -> Option<ServerStats> {
        let (tx, mut rx) = mpsc::channel(1);
        if self.tx.send(StatsCommand::GetSnapshot(tx)).await.is_err() {
            return None;
        }
        rx.recv().await
    }

    /// 记录文件上传
    pub async fn record_upload(&self, path: String, bytes: u64) {
        let _ = self
            .tx
            .send(StatsCommand::RecordUpload { path, bytes })
            .await;
    }

    /// 记录文件下载
    pub async fn record_download(&self, path: String, bytes: u64) {
        let _ = self
            .tx
            .send(StatsCommand::RecordDownload { path, bytes })
            .await;
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
        let _ = self
            .tx
            .send(StatsCommand::UpdateConnectionCount { count })
            .await;
    }
}

/// 统计信息Actor工作者
pub struct StatsActorWorker {
    rx: mpsc::Receiver<StatsCommand>,
    stats: Arc<RwLock<ServerStats>>,
}

impl StatsActorWorker {
    fn new(rx: mpsc::Receiver<StatsCommand>) -> Self {
        Self {
            rx,
            stats: Arc::new(RwLock::new(ServerStats::default())),
        }
    }

    /// 运行Actor主循环
    pub async fn run(mut self) {
        trace!("StatsActor started");

        while let Some(cmd) = self.rx.recv().await {
            match cmd {
                StatsCommand::GetStats(tx) => {
                    let stats = self.stats.read().await.clone();
                    let _ = tx.send(stats);
                }
                StatsCommand::GetSnapshot(tx) => {
                    // 尝试读取，如果被锁定则返回默认
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
                    debug!(
                        upload_count = stats.total_uploads,
                        bytes_received = stats.total_bytes_received,
                        file = %path,
                        "File uploaded"
                    );
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
                    let old_count = stats.active_connections;
                    stats.active_connections = count;
                    if old_count != count {
                        trace!(
                            old = old_count,
                            new = count,
                            "Connection count updated"
                        );
                    }
                }
            }
        }

        trace!("StatsActor stopped");
    }
}

/// 同步统计快照（用于快速读取，无需等待）
#[derive(Debug, Clone)]
pub struct StatsSnapshot {
    inner: Arc<RwLock<ServerStats>>,
}

impl StatsSnapshot {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(ServerStats::default())),
        }
    }

    /// 尝试获取统计（非阻塞）
    pub fn try_get(&self) -> Option<ServerStats> {
        self.inner.try_read().ok().map(|g| g.clone())
    }

    /// 异步获取统计
    pub async fn get(&self) -> ServerStats {
        self.inner.read().await.clone()
    }

    /// 更新统计（内部使用）
    pub async fn update(&self, stats: ServerStats) {
        let mut guard = self.inner.write().await;
        *guard = stats;
    }
}

impl Default for StatsSnapshot {
    fn default() -> Self {
        Self::new()
    }
}
