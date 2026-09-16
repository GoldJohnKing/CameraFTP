// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

// 文件系统监听仅在 Windows 平台启用
// Android 不使用文件系统监听
//
// 并发模型：notify 回调线程只做事件分类，把事件 try_send 进 mpsc 通道
//（容量 1000；满时告警并丢弃该事件，索引最终一致，下次扫描可补）。
// 事件循环对 Created/Renamed 重事件（wait_for_file_ready 秒级等待 +
// EXIF 全文件解析）每事件独立 tokio::spawn 并发处理，避免一个慢文件
// 阻塞整批事件；Deleted 轻事件保持串行 await。正确性不依赖处理顺序：
// add_file 在写锁内做查重-回填幂等处理，并发重复的 Created 对同一
// 文件第二次进入时直接跳过或回填，不会产生重复条目。
#![cfg(target_os = "windows")]

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use notify::{Event, RecursiveMode, Watcher, RecommendedWatcher};
use tokio::sync::mpsc::{channel, Sender};
use tracing::{info, debug, error, warn};

use crate::file_index::FileIndexService;
use crate::constants::FILE_READY_TIMEOUT_SECS;
use crate::utils::wait_for_file_ready;

/// 文件系统事件类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileSystemEvent {
    /// 文件创建
    Created(PathBuf),
    /// 文件删除
    Deleted(PathBuf),
    /// 文件重命名
    Renamed { from: PathBuf, to: PathBuf },
}

/// Windows 文件系统监听器
/// 
/// 使用 notify crate 的 ReadDirectoryChangesW 后端
pub struct FileWatcher {
    watcher: Option<RecommendedWatcher>,
    watch_path: PathBuf,
    event_sender: Option<Sender<FileSystemEvent>>,
}

impl FileWatcher {
    /// 创建新的文件监听器
    pub fn new(watch_path: PathBuf) -> Self {
        Self {
            watcher: None,
            watch_path,
            event_sender: None,
        }
    }

    /// 开始监听文件系统事件
    /// 
    /// # Arguments
    /// * `file_index` - 文件索引服务，用于同步索引
    /// 
    /// # Platform Support
    /// - Windows: 使用 notify crate
    pub async fn start(&mut self, file_index: Arc<FileIndexService>) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        if self.watcher.is_some() {
            info!("File watcher already running");
            return Ok(true);
        }

        let (tx, rx) = channel::<FileSystemEvent>(1000);
        self.event_sender = Some(tx.clone());

        // 创建 notify watcher
        let watcher_tx = tx.clone();
        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                match res {
                    Ok(event) => {
                        Self::handle_notify_event(event, &watcher_tx);
                    }
                    Err(e) => {
                        error!("File watcher error: {}", e);
                    }
                }
            },
            // 仅 PollWatcher 生效；Windows ReadDirectoryChangesW 后端忽略
            //（保留供非 Windows 调试场景）
            notify::Config::default()
                .with_poll_interval(Duration::from_secs(2))
                .with_compare_contents(true),
        )?;

        // 开始监听
        watcher.watch(&self.watch_path, RecursiveMode::Recursive)?;
        self.watcher = Some(watcher);

        info!("File watcher started for: {:?}", self.watch_path);

        // 启动事件处理任务（并发模型见文件顶部注释）
        let file_index_clone = file_index.clone();
        tokio::spawn(async move {
            Self::run_event_loop(rx, file_index_clone).await;
        });

        Ok(true)
    }

    /// 停止监听
    pub fn stop(&mut self) {
        if let Some(watcher) = self.watcher.take() {
            drop(watcher);
            info!("File watcher stopped");
        }
        self.event_sender = None;
    }

    /// 事件循环：重事件（Created/Renamed，需 wait_for_file_ready 秒级
    /// 等待 + EXIF 全文件解析）每事件独立 tokio::spawn 并发处理，防止
    /// 一个慢文件阻塞整批事件；轻事件（Deleted，仅索引写锁内移除）
    /// 保持串行 await。
    async fn run_event_loop(
        mut rx: tokio::sync::mpsc::Receiver<FileSystemEvent>,
        file_index: Arc<FileIndexService>,
    ) {
        while let Some(event) = rx.recv().await {
            match event {
                FileSystemEvent::Deleted(_) => {
                    Self::process_event(event, Arc::clone(&file_index)).await;
                }
                FileSystemEvent::Created(_) | FileSystemEvent::Renamed { .. } => {
                    tokio::spawn(Self::process_event(event, Arc::clone(&file_index)));
                }
            }
        }
    }

    /// 处理 notify 事件，转换为内部事件格式
    fn handle_notify_event(event: Event, tx: &Sender<FileSystemEvent>) {
        use notify::event::{EventKind, ModifyKind, RenameMode};

        debug!("Raw notify event: {:?}", event);

        match event.kind {
            EventKind::Create(_) => {
                for path in &event.paths {
                    if crate::image_utils::is_supported_image(path)
                        && tx.try_send(FileSystemEvent::Created(path.clone())).is_err() {
                            warn!("file watcher channel full, event dropped: {:?}", path);
                        }
                }
            }
            // 重命名旧路径（From）→ 等同于删除
            EventKind::Modify(ModifyKind::Name(RenameMode::From)) => {
                for path in &event.paths {
                    if crate::image_utils::is_supported_image(path)
                        && tx.try_send(FileSystemEvent::Deleted(path.clone())).is_err() {
                            warn!("file watcher channel full, event dropped: {:?}", path);
                        }
                }
            }
            // 重命名新路径（To）→ 等同于创建
            EventKind::Modify(ModifyKind::Name(RenameMode::To)) => {
                for path in &event.paths {
                    if crate::image_utils::is_supported_image(path)
                        && tx.try_send(FileSystemEvent::Created(path.clone())).is_err() {
                            warn!("file watcher channel full, event dropped: {:?}", path);
                        }
                }
            }
            // 单事件同时携带 from + to（某些后端）
            EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => {
                if event.paths.len() == 2 {
                    let from = &event.paths[0];
                    let to = &event.paths[1];
                    if (crate::image_utils::is_supported_image(from)
                        || crate::image_utils::is_supported_image(to))
                        && tx
                            .try_send(FileSystemEvent::Renamed {
                                from: from.clone(),
                                to: to.clone(),
                            })
                            .is_err()
                        {
                            warn!(
                                "file watcher channel full, event dropped: {:?} -> {:?}",
                                from, to
                            );
                        }
                }
            }
            // 其他修改事件（内容/属性/时间戳）不需要索引更新
            EventKind::Modify(_) => {}
            EventKind::Remove(_) => {
                for path in &event.paths {
                    if crate::image_utils::is_supported_image(path)
                        && tx.try_send(FileSystemEvent::Deleted(path.clone())).is_err() {
                            warn!("file watcher channel full, event dropped: {:?}", path);
                        }
                }
            }
            _ => {
                // 其他未知事件：不做处理
            }
        }
    }

    /// 处理文件系统事件并同步到索引
    async fn process_event(event: FileSystemEvent, file_index: Arc<FileIndexService>) {
        match event {
            FileSystemEvent::Created(path) => {
                debug!("File created: {:?}", path);
                // 等待文件就绪（而非固定延迟）；超时不再丢弃事件，降级为
                // 尽力而为索引——陈旧/半写条目由 add_file 的 EXIF 回填机制善后
                if !wait_for_file_ready(&path, Duration::from_secs(FILE_READY_TIMEOUT_SECS)).await {
                    warn!("File not ready after timeout, indexing best-effort: {:?}", path);
                }
                if let Err(e) = file_index.add_file(path.clone()).await {
                    warn!("Failed to add file to index: {}", e);
                } else {
                    info!("File added to index via watcher: {:?}", path);
                }
            }
            FileSystemEvent::Deleted(path) => {
                debug!("File deleted: {:?}", path);
                
                match file_index.remove_file(&path).await {
                    Ok(true) => {
                        info!("File removed from index via watcher: {:?}", path);
                    }
                    Ok(false) => {
                        // 文件不在索引中，忽略（幂等性保证）
                        debug!("File not in index, ignoring: {:?}", path);
                    }
                    Err(e) => {
                        error!("Failed to remove file from index: {}", e);
                    }
                }
            }
            FileSystemEvent::Renamed { from, to } => {
                debug!("File renamed: {:?} -> {:?}", from, to);
                
                // 先移除旧路径
                if let Ok(true) = file_index.remove_file(&from).await {
                    info!("Removed old path from index: {:?}", from);
                }
                
                // 等待新路径文件就绪；超时同样降级为尽力而为索引（EXIF 回填善后）
                if !wait_for_file_ready(&to, Duration::from_secs(FILE_READY_TIMEOUT_SECS)).await {
                    warn!("Renamed file not ready after timeout, indexing best-effort: {:?}", to);
                }
                if let Err(e) = file_index.add_file(to.clone()).await {
                    warn!("Failed to add renamed file to index: {}", e);
                } else {
                    info!("Added renamed file to index: {:?}", to);
                }
            }
        }
    }
}

impl Drop for FileWatcher {
    fn drop(&mut self) {
        if self.watcher.take().is_some() {
            tracing::info!("File watcher stopped");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config_service::ConfigService;
    use crate::file_index::FileIndexService;
    use notify::event::{AccessKind, CreateKind, DataChange, ModifyKind, RemoveKind, RenameMode};
    use std::sync::Arc;

    /// 构造仅驻留内存配置的文件索引服务（ConfigService::new_with_path 不做磁盘 IO）
    fn make_file_index() -> Arc<FileIndexService> {
        let config_service = Arc::new(ConfigService::new_with_path(
            std::env::temp_dir().join("cameraftp-watcher-test-config.json"),
        ));
        Arc::new(FileIndexService::new(config_service))
    }

    /// 用构造的 notify 事件驱动分类逻辑，同步收集转换出的内部事件
    fn classify(kind: notify::event::EventKind, paths: Vec<PathBuf>) -> Vec<FileSystemEvent> {
        let (tx, mut rx) = channel::<FileSystemEvent>(32);
        // notify 8.0：Event::new 只收 EventKind；paths/attrs 为公有字段，经结构体字面量设置
        let event = notify::Event {
            kind,
            paths,
            ..Default::default()
        };
        FileWatcher::handle_notify_event(event, &tx);
        drop(tx);

        let mut events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }
        events
    }

    // ---- notify 事件 → 内部事件的分类映射 ----

    #[test]
    fn create_events_map_supported_images_to_created() {
        let jpg = PathBuf::from(r"C:\photos\a.jpg");
        let raw = PathBuf::from(r"C:\photos\b.NEF");

        assert_eq!(
            classify(notify::event::EventKind::Create(CreateKind::File), vec![jpg.clone()]),
            vec![FileSystemEvent::Created(jpg)]
        );
        // RAW 扩展名（大小写不敏感）同样视为受支持图片
        assert_eq!(
            classify(notify::event::EventKind::Create(CreateKind::File), vec![raw.clone()]),
            vec![FileSystemEvent::Created(raw)]
        );
    }

    #[test]
    fn create_events_ignore_non_media_extensions() {
        for name in ["C:\\photos\\notes.txt", "C:\\photos\\clip.mp4", "C:\\photos\\README"] {
            assert!(
                classify(
                    notify::event::EventKind::Create(CreateKind::File),
                    vec![PathBuf::from(name)],
                )
                .is_empty(),
                "{} must not be forwarded",
                name
            );
        }
    }

    #[test]
    fn remove_events_map_to_deleted() {
        let jpg = PathBuf::from(r"C:\photos\a.jpg");
        assert_eq!(
            classify(notify::event::EventKind::Remove(RemoveKind::File), vec![jpg.clone()]),
            vec![FileSystemEvent::Deleted(jpg)]
        );
    }

    #[test]
    fn rename_from_maps_to_deleted_and_rename_to_maps_to_created() {
        let from = PathBuf::from(r"C:\photos\a.jpg");
        let to = PathBuf::from(r"C:\photos\b.jpg");

        // 重命名旧路径（From）→ 等同于删除
        assert_eq!(
            classify(
                notify::event::EventKind::Modify(ModifyKind::Name(RenameMode::From)),
                vec![from.clone()],
            ),
            vec![FileSystemEvent::Deleted(from)]
        );
        // 重命名新路径（To）→ 等同于创建
        assert_eq!(
            classify(
                notify::event::EventKind::Modify(ModifyKind::Name(RenameMode::To)),
                vec![to.clone()],
            ),
            vec![FileSystemEvent::Created(to)]
        );
    }

    #[test]
    fn rename_both_requires_two_paths_and_either_side_media() {
        let from = PathBuf::from(r"C:\photos\a.jpg");
        let to = PathBuf::from(r"C:\photos\b.jpg");
        let txt_from = PathBuf::from(r"C:\photos\a.txt");
        let txt_to = PathBuf::from(r"C:\photos\b.txt");
        let both = notify::event::EventKind::Modify(ModifyKind::Name(RenameMode::Both));

        // 两个图片路径 → Renamed（移除旧 + 新增新）
        assert_eq!(
            classify(both.clone(), vec![from.clone(), to.clone()]),
            vec![FileSystemEvent::Renamed { from: from.clone(), to: to.clone() }]
        );
        // 任一侧是图片即转发（如 txt 重命名为 jpg 进入目录）
        assert_eq!(
            classify(both.clone(), vec![txt_from.clone(), to.clone()]),
            vec![FileSystemEvent::Renamed { from: txt_from.clone(), to: to.clone() }]
        );
        // 两侧都不是图片 → 忽略
        assert!(classify(both.clone(), vec![txt_from, txt_to]).is_empty());
        // 只携带一个路径的 Both 事件（形状不完整）→ 忽略
        assert!(classify(both, vec![from]).is_empty());
    }

    #[test]
    fn content_access_and_unknown_events_are_ignored() {
        let jpg = PathBuf::from(r"C:\photos\a.jpg");

        // 内容修改不触发索引更新
        assert!(classify(
            notify::event::EventKind::Modify(ModifyKind::Data(DataChange::Any)),
            vec![jpg.clone()],
        )
        .is_empty());
        // 其他形式的修改（属性/时间戳/未细分）不触发
        assert!(classify(
            notify::event::EventKind::Modify(ModifyKind::Any),
            vec![jpg.clone()],
        )
        .is_empty());
        // 访问事件（读取/关闭）不触发
        assert!(classify(
            notify::event::EventKind::Access(AccessKind::Any),
            vec![jpg.clone()],
        )
        .is_empty());
        // 未知事件类型不触发
        assert!(classify(notify::event::EventKind::Other, vec![jpg]).is_empty());
    }

    // ---- 事件 → 索引 的同步处理（process_event）----

    fn write_image(dir: &std::path::Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, b"fake-jpeg-content").expect("write image file");
        path
    }

    /// 事件循环将重事件 spawn 到独立任务后，测试无法直接 await 其完成，
    /// 必须以最终状态轮询代替固定 sleep：50ms 间隔，10s 超时断言。
    async fn wait_for_count(service: &Arc<FileIndexService>, n: usize) {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        loop {
            let count = service.get_file_count().await;
            if count >= n {
                return;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "timed out waiting for file count to reach {} (current: {})",
                n,
                count
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    #[tokio::test]
    async fn event_loop_dispatches_created_concurrently_and_deleted_serially() {
        // 钉住事件循环的并发模型：Created/Renamed 经独立 spawn 并发进入
        // 索引（两个重事件不再串行等待彼此），Deleted 串行移除。用轮询
        // 等待最终状态，避免对 spawn 调度的时序假设。
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let file_index = make_file_index();
        let (tx, rx) = channel::<FileSystemEvent>(16);

        let loop_service = Arc::clone(&file_index);
        tokio::spawn(FileWatcher::run_event_loop(rx, loop_service));

        let a = write_image(temp_dir.path(), "a.jpg");
        let b = write_image(temp_dir.path(), "b.jpg");

        tx.send(FileSystemEvent::Created(a.clone())).await.unwrap();
        tx.send(FileSystemEvent::Created(b.clone())).await.unwrap();
        wait_for_count(&file_index, 2).await;

        // 轻事件（Deleted）在两个重事件入索引之后送达：串行移除必须生效
        tx.send(FileSystemEvent::Deleted(b.clone())).await.unwrap();
        drop(tx);

        // 轮询直到 b 确实被移除（count >= 1 不足以证明删除已发生）
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        loop {
            let files = file_index.get_files().await;
            if !files.iter().any(|f| f.path == b) {
                break;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "timed out waiting for Deleted event to remove the file"
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        let files = file_index.get_files().await;
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, a, "only the surviving file may remain indexed");
    }

    #[tokio::test]
    async fn process_created_adds_ready_file_to_index() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let file_index = make_file_index();
        let path = write_image(temp_dir.path(), "watched.jpg");

        FileWatcher::process_event(FileSystemEvent::Created(path.clone()), Arc::clone(&file_index))
            .await;

        assert_eq!(file_index.get_file_count().await, 1);
        let files = file_index.get_files().await;
        assert_eq!(files[0].path, path);
    }

    #[tokio::test]
    async fn process_deleted_removes_from_index_and_is_idempotent() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let file_index = make_file_index();
        let path = write_image(temp_dir.path(), "deleted.jpg");
        file_index.add_file(path.clone()).await.expect("seed index");

        FileWatcher::process_event(FileSystemEvent::Deleted(path.clone()), Arc::clone(&file_index))
            .await;
        assert_eq!(file_index.get_file_count().await, 0);

        // 索引中不存在的文件再次删除：幂等（Ok(false)），无 panic
        FileWatcher::process_event(FileSystemEvent::Deleted(path), Arc::clone(&file_index)).await;
        assert_eq!(file_index.get_file_count().await, 0);
    }

    #[tokio::test]
    async fn process_renamed_moves_index_entry_to_new_path() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let file_index = make_file_index();
        let from = write_image(temp_dir.path(), "before.jpg");
        let to = write_image(temp_dir.path(), "after.jpg");
        file_index.add_file(from.clone()).await.expect("seed index");

        FileWatcher::process_event(
            FileSystemEvent::Renamed { from, to: to.clone() },
            Arc::clone(&file_index),
        )
        .await;

        assert_eq!(
            file_index.get_file_count().await, 1,
            "rename must move the entry, not duplicate it"
        );
        let files = file_index.get_files().await;
        assert_eq!(files[0].path, to);
    }

    #[tokio::test]
    async fn process_created_for_missing_file_times_out_but_attempts_best_effort_index() {
        // 文件始终不落地 → wait_for_file_ready 在 FILE_READY_TIMEOUT_SECS（5s）后放弃，
        // 超时分支不再丢弃事件而是 best-effort 调 add_file；add_file 因 metadata Err
        // 返回 Err（warn 路径），索引保持干净——无 panic、无 ghost 条目（~5s）
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let file_index = make_file_index();
        let ghost = temp_dir.path().join("ghost.jpg");

        FileWatcher::process_event(FileSystemEvent::Created(ghost), Arc::clone(&file_index)).await;

        assert_eq!(file_index.get_file_count().await, 0);
    }

    #[tokio::test]
    async fn process_created_times_out_for_changing_file_but_still_indexes_it() {
        // 探测超时但文件真实存在（mtime 在整个探测窗口内持续变化，模拟极慢写入）：
        // 降级路径必须仍调用 add_file 尽力索引，而不是跳过事件（钉住 R1 的
        // best-effort 语义——陈旧条目交由 EXIF 回填机制善后，测试耗时 ~6s）
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let file_index = make_file_index();
        let path = write_image(temp_dir.path(), "slow-write.jpg");

        // 后台持续拨动 mtime（len 不变），让 (len, mtime) 签名在整个 5s 探测窗口内
        // 永不稳定 → wait_for_file_ready 必然走超时分支
        let touch_path = path.clone();
        let bumper = tokio::spawn(async move {
            for i in 0..80u32 {
                tokio::time::sleep(Duration::from_millis(80)).await;
                let bumped = std::time::SystemTime::now()
                    + std::time::Duration::from_secs(1)
                    + std::time::Duration::from_millis(u64::from(i));
                let _ = filetime::set_file_mtime(
                    &touch_path,
                    filetime::FileTime::from_system_time(bumped),
                );
            }
        });

        FileWatcher::process_event(FileSystemEvent::Created(path.clone()), Arc::clone(&file_index))
            .await;

        assert_eq!(
            file_index.get_file_count().await, 1,
            "timeout must degrade to best-effort add_file, not skip the event"
        );
        let files = file_index.get_files().await;
        assert_eq!(files[0].path, path, "degraded index must contain the slow file");

        // 收尾后台拨动任务，避免 tempdir 清理与句柄竞争
        bumper.abort();
    }

    // ---- 生命周期 ----

    #[tokio::test]
    async fn watcher_start_is_idempotent_and_stop_is_idempotent() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let file_index = make_file_index();

        let mut watcher = FileWatcher::new(temp_dir.path().to_path_buf());
        assert!(matches!(
            watcher.start(Arc::clone(&file_index)).await,
            Ok(true)
        ));
        // 已在运行时重复 start：直接 Ok(true)，不重建 watcher
        assert!(matches!(
            watcher.start(Arc::clone(&file_index)).await,
            Ok(true)
        ));

        watcher.stop();
        watcher.stop(); // 重复 stop 不得 panic
    }
}

