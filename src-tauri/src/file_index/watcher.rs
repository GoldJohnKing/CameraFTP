// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

// 文件系统监听仅在 Windows 平台启用
// Android 不使用文件系统监听
//
// 并发模型：notify 回调线程只做事件分类，把事件 try_send 进 mpsc 通道
//（容量 1000；满时告警丢弃并置位 needs_rescan——事件循环在通道排空后
// check-and-swap 触发一次合并重扫收敛丢弃，见 run_event_loop）。
// 事件循环对 Created/Renamed 重事件（wait_for_file_ready 秒级等待 +
// EXIF 全文件解析）每事件独立 tokio::spawn 并发处理（信号量限流，见
// run_event_loop），避免一个慢文件阻塞整批事件；Deleted 轻事件保持
// 串行 await。处理顺序存在真实的乱序窗口：串行的 Deleted 可能赶在
// 并发 spawn 的 Created 提交之前被处理（remove_file 落空 Ok(false)），
// 随后该 Created 提交即成幽灵条目——由 add_file 写锁内的提交前存在性
// 检查兜底（见 FileIndexService::add_file，原子根位）；并发重复的
// Created 则由 add_file 写锁内的查重-回填幂等吸收，不会产生重复条目。
#![cfg(target_os = "windows")]

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::{channel, Sender};
use tracing::{debug, error, info, warn};

use crate::constants::{FILE_READY_TIMEOUT_SECS, INDEX_EVENT_CONCURRENCY};
use crate::file_index::FileIndexService;
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
    pub async fn start(
        &mut self,
        file_index: Arc<FileIndexService>,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        if self.watcher.is_some() {
            info!("File watcher already running");
            return Ok(true);
        }

        let (tx, rx) = channel::<FileSystemEvent>(1000);
        self.event_sender = Some(tx.clone());

        // 丢弃事件收敛标志：notify 回调线程 try_send 失败时置位，事件
        // 循环在通道排空后 check-and-swap 触发一次合并重扫（见
        // run_event_loop 尾部）
        let needs_rescan = Arc::new(AtomicBool::new(false));

        // 创建 notify watcher
        let watcher_tx = tx.clone();
        let watcher_needs_rescan = Arc::clone(&needs_rescan);
        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| match res {
                Ok(event) => {
                    Self::handle_notify_event(event, &watcher_tx, &watcher_needs_rescan);
                }
                Err(e) => {
                    error!("File watcher error: {}", e);
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
            Self::run_event_loop(rx, file_index_clone, needs_rescan).await;
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
    /// 等待 + EXIF 全文件解析）每事件独立 tokio::spawn 并发处理（信号量
    /// 限流），防止一个慢文件阻塞整批事件；轻事件（Deleted，仅索引写锁
    /// 内移除）保持串行 await。
    ///
    /// 丢弃事件收敛（B2）：notify 回调线程通道满时丢弃事件并置位
    /// needs_rescan；本循环在每批事件排空（try_recv 返回 Empty）后
    /// check-and-swap 该标志，置位过则执行一次合并重扫（scan_directory）。
    async fn run_event_loop(
        mut rx: tokio::sync::mpsc::Receiver<FileSystemEvent>,
        file_index: Arc<FileIndexService>,
        needs_rescan: Arc<AtomicBool>,
    ) {
        // 重事件并发上限 INDEX_EVENT_CONCURRENCY（constants.rs）：与扫描侧
        // INDEX_SCAN_CONCURRENCY 的 2× 关系是有意的不变式——事件通道含
        // wait_for_file_ready 等待期，吞吐上界对齐扫描。
        // 重事件任务各自经 spawn_blocking 做 EXIF 全文件解析，若千级
        // 文件批量到达（相机连拍导入）时无上限 spawn，会占满 tokio 阻塞
        // 池——EXIF 解析排队反而拖慢包括本批在内的一切阻塞任务。
        let semaphore = Arc::new(tokio::sync::Semaphore::new(INDEX_EVENT_CONCURRENCY));
        while let Some(event) = rx.recv().await {
            Self::dispatch_event(event, &file_index, &semaphore).await;

            // 排空积压：try_recv 连续取到 Empty 视为排空（Disconnected 时
            // 队列也已取尽，同样落入排空分支）。丢弃只发生在通道满时，满
            // 意味着必有积压事件待处理，因此任何置位终将等到一次排空观察。
            loop {
                match rx.try_recv() {
                    Ok(event) => Self::dispatch_event(event, &file_index, &semaphore).await,
                    Err(tokio::sync::mpsc::error::TryRecvError::Empty)
                    | Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => break,
                }
            }

            // check-and-swap 即重入守卫：只有把 true 换成 false 的一方执行
            // 扫描；扫描期间若再次溢出丢弃，flag 被重新置位，下一轮排空
            // 再次收敛。合并重扫与在飞 spawn 的重事件（add_file）并发安全
            // ——merge_scan_result（service.rs）保留扫描期间新增的条目，
            // 提交语义正确合并不丢数据。扫描失败则恢复置位，下次排空重试。
            if needs_rescan.swap(false, Ordering::AcqRel) {
                info!("file watcher dropped events detected, running merged rescan");
                if let Err(e) = file_index.scan_directory().await {
                    warn!(
                        error = %e,
                        "merged rescan after dropped events failed, will retry on next drain"
                    );
                    needs_rescan.store(true, Ordering::Release);
                }
            }
        }
    }

    /// 单事件分发：重事件（Created/Renamed）spawn 并发处理（信号量限流，
    /// permit 随任务结束释放）；轻事件（Deleted）串行 await
    async fn dispatch_event(
        event: FileSystemEvent,
        file_index: &Arc<FileIndexService>,
        semaphore: &Arc<tokio::sync::Semaphore>,
    ) {
        match event {
            FileSystemEvent::Deleted(_) => {
                Self::process_event(event, Arc::clone(file_index)).await;
            }
            FileSystemEvent::Created(_) | FileSystemEvent::Renamed { .. } => {
                let semaphore = Arc::clone(semaphore);
                let file_index = Arc::clone(file_index);
                tokio::spawn(async move {
                    // 先取许可再处理；permit guard 随任务结束自动释放。
                    // 信号量从不 close，Err 分支不可达。
                    let _permit = semaphore
                        .acquire_owned()
                        .await
                        .expect("watcher semaphore is never closed");
                    Self::process_event(event, file_index).await;
                });
            }
        }
    }

    /// 处理 notify 事件，转换为内部事件格式
    ///
    /// 通道满（try_send 失败）时丢弃该事件并置位 needs_rescan——事件循环
    /// 在通道排空后执行一次合并重扫收敛丢弃（见 run_event_loop）。
    fn handle_notify_event(event: Event, tx: &Sender<FileSystemEvent>, needs_rescan: &AtomicBool) {
        use notify::event::{EventKind, ModifyKind, RenameMode};

        debug!("Raw notify event: {:?}", event);

        match event.kind {
            EventKind::Create(_) => {
                for path in &event.paths {
                    if crate::image_utils::is_supported_image(path)
                        && tx.try_send(FileSystemEvent::Created(path.clone())).is_err()
                    {
                        warn!("file watcher channel full, event dropped: {:?}", path);
                        needs_rescan.store(true, Ordering::Release);
                    }
                }
            }
            // 重命名旧路径（From）→ 等同于删除
            EventKind::Modify(ModifyKind::Name(RenameMode::From)) => {
                for path in &event.paths {
                    if crate::image_utils::is_supported_image(path)
                        && tx.try_send(FileSystemEvent::Deleted(path.clone())).is_err()
                    {
                        warn!("file watcher channel full, event dropped: {:?}", path);
                        needs_rescan.store(true, Ordering::Release);
                    }
                }
            }
            // 重命名新路径（To）→ 等同于创建
            EventKind::Modify(ModifyKind::Name(RenameMode::To)) => {
                for path in &event.paths {
                    if crate::image_utils::is_supported_image(path)
                        && tx.try_send(FileSystemEvent::Created(path.clone())).is_err()
                    {
                        warn!("file watcher channel full, event dropped: {:?}", path);
                        needs_rescan.store(true, Ordering::Release);
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
                        needs_rescan.store(true, Ordering::Release);
                    }
                }
            }
            // 其他修改事件（内容/属性/时间戳）不需要索引更新
            EventKind::Modify(_) => {}
            EventKind::Remove(_) => {
                for path in &event.paths {
                    if crate::image_utils::is_supported_image(path)
                        && tx.try_send(FileSystemEvent::Deleted(path.clone())).is_err()
                    {
                        warn!("file watcher channel full, event dropped: {:?}", path);
                        needs_rescan.store(true, Ordering::Release);
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
                // 尽力而为索引——陈旧/半写条目由 add_file 的 EXIF 回填机制
                // 善后（FTP 双通道已各索引一次；慢文件由启动/换路径扫描兜底，
                // watcher 侧不再做延迟重试）。幽灵/归属防御在 add_file 内部
                //（写锁内提交前存在性检查 + save_path 归属校验）。
                if !wait_for_file_ready(&path, Duration::from_secs(FILE_READY_TIMEOUT_SECS)).await {
                    warn!(
                        "File not ready after timeout, indexing best-effort: {:?}",
                        path
                    );
                }
                if let Err(e) = file_index.add_file(path).await {
                    warn!("Failed to add file to index: {}", e);
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

                // 等待新路径文件就绪；超时同样降级为尽力而为索引
                //（EXIF 回填善后；幽灵/归属防御在 add_file 内部）
                if !wait_for_file_ready(&to, Duration::from_secs(FILE_READY_TIMEOUT_SECS)).await {
                    warn!(
                        "Renamed file not ready after timeout, indexing best-effort: {:?}",
                        to
                    );
                }
                if let Err(e) = file_index.add_file(to).await {
                    warn!("Failed to add renamed file to index: {}", e);
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

    /// 构造配置了 save_path（= 给定目录）的文件索引服务
    ///（ConfigService::new_with_path 不做磁盘 IO）。add_file 的归属校验
    ///（service.rs S4）要求被索引文件位于当前 save_path 之下，目录内
    /// 落盘的测试文件全部满足。
    fn make_file_index(save_path: &std::path::Path) -> Arc<FileIndexService> {
        let config_service = Arc::new(ConfigService::new_with_path(
            save_path.join("watcher-test-config.json"),
        ));
        config_service
            .mutate_and_persist(|config| {
                config.save_path = save_path.to_path_buf();
            })
            .expect("persist config");
        Arc::new(FileIndexService::new(config_service))
    }

    /// 用构造的 notify 事件驱动分类逻辑，同步收集转换出的内部事件
    fn classify(kind: notify::event::EventKind, paths: Vec<PathBuf>) -> Vec<FileSystemEvent> {
        let (tx, mut rx) = channel::<FileSystemEvent>(32);
        let needs_rescan = AtomicBool::new(false);
        // notify 8.0：Event::new 只收 EventKind；paths/attrs 为公有字段，经结构体字面量设置
        let event = notify::Event {
            kind,
            paths,
            ..Default::default()
        };
        FileWatcher::handle_notify_event(event, &tx, &needs_rescan);
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
            classify(
                notify::event::EventKind::Create(CreateKind::File),
                vec![jpg.clone()]
            ),
            vec![FileSystemEvent::Created(jpg)]
        );
        // RAW 扩展名（大小写不敏感）同样视为受支持图片
        assert_eq!(
            classify(
                notify::event::EventKind::Create(CreateKind::File),
                vec![raw.clone()]
            ),
            vec![FileSystemEvent::Created(raw)]
        );
    }

    #[test]
    fn create_events_ignore_non_media_extensions() {
        for name in [
            "C:\\photos\\notes.txt",
            "C:\\photos\\clip.mp4",
            "C:\\photos\\README",
        ] {
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
            classify(
                notify::event::EventKind::Remove(RemoveKind::File),
                vec![jpg.clone()]
            ),
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
            vec![FileSystemEvent::Renamed {
                from: from.clone(),
                to: to.clone()
            }]
        );
        // 任一侧是图片即转发（如 txt 重命名为 jpg 进入目录）
        assert_eq!(
            classify(both.clone(), vec![txt_from.clone(), to.clone()]),
            vec![FileSystemEvent::Renamed {
                from: txt_from.clone(),
                to: to.clone()
            }]
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
        let file_index = make_file_index(temp_dir.path());
        let (tx, rx) = channel::<FileSystemEvent>(16);
        let needs_rescan = Arc::new(AtomicBool::new(false));

        let loop_service = Arc::clone(&file_index);
        tokio::spawn(FileWatcher::run_event_loop(
            rx,
            loop_service,
            needs_rescan,
        ));

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
        assert_eq!(
            files[0].path, a,
            "only the surviving file may remain indexed"
        );
    }

    #[tokio::test]
    async fn process_created_adds_ready_file_to_index() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let file_index = make_file_index(temp_dir.path());
        let path = write_image(temp_dir.path(), "watched.jpg");

        FileWatcher::process_event(
            FileSystemEvent::Created(path.clone()),
            Arc::clone(&file_index),
        )
        .await;

        assert_eq!(file_index.get_file_count().await, 1);
        let files = file_index.get_files().await;
        assert_eq!(files[0].path, path);
    }

    #[tokio::test]
    async fn process_deleted_removes_from_index_and_is_idempotent() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let file_index = make_file_index(temp_dir.path());
        let path = write_image(temp_dir.path(), "deleted.jpg");
        file_index.add_file(path.clone()).await.expect("seed index");

        FileWatcher::process_event(
            FileSystemEvent::Deleted(path.clone()),
            Arc::clone(&file_index),
        )
        .await;
        assert_eq!(file_index.get_file_count().await, 0);

        // 索引中不存在的文件再次删除：幂等（Ok(false)），无 panic
        FileWatcher::process_event(FileSystemEvent::Deleted(path), Arc::clone(&file_index)).await;
        assert_eq!(file_index.get_file_count().await, 0);
    }

    #[tokio::test]
    async fn process_renamed_moves_index_entry_to_new_path() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let file_index = make_file_index(temp_dir.path());
        let from = write_image(temp_dir.path(), "before.jpg");
        let to = write_image(temp_dir.path(), "after.jpg");
        file_index.add_file(from.clone()).await.expect("seed index");

        FileWatcher::process_event(
            FileSystemEvent::Renamed {
                from,
                to: to.clone(),
            },
            Arc::clone(&file_index),
        )
        .await;

        assert_eq!(
            file_index.get_file_count().await,
            1,
            "rename must move the entry, not duplicate it"
        );
        let files = file_index.get_files().await;
        assert_eq!(files[0].path, to);
    }

    // ---- Created-vs-Deleted 乱序窗口（X1）----

    #[tokio::test]
    async fn deleted_before_created_completion_leaves_no_ghost_entry() {
        // X1 交错语义钉住：Deleted 在 Created 完成之前被处理
        //（remove_file 落空 Ok(false)）时，最终索引不得残留已消失文件。
        //
        // 交错构造：mtime 持续拨动（间隔 80ms < 200ms 稳定窗口）让
        // Created 任务确定性地停在 wait_for_file_ready（在飞、文件在盘），
        // 此刻处理 Deleted；随后停拨并删除磁盘文件，放行 Created 走完
        // 超时降级（~5s）。
        //
        // 注：本交错下 Created 的 add_file 在文件消失后才执行
        //（metadata Err），幽灵不会真正产生——它钉住的是"乱序送达时
        // 最终态无幽灵"这一不变量；"提交前一刻才消失"的窄窗（幽灵在
        // 写锁段顶部被存在性检查拦截）由下方
        // created_ghost_check_inside_write_lock_blocks_vanished_file
        // 以写锁卡点确定性钉住。两个测试合起来覆盖乱序的两个相位。
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let file_index = make_file_index(temp_dir.path());
        let path = write_image(temp_dir.path(), "ghost-race.jpg");

        let bumper_path = path.clone();
        let bumper = tokio::spawn(async move {
            for i in 0..200u32 {
                tokio::time::sleep(Duration::from_millis(80)).await;
                let bumped = std::time::SystemTime::now()
                    + std::time::Duration::from_secs(1)
                    + std::time::Duration::from_millis(u64::from(i));
                let _ = filetime::set_file_mtime(
                    &bumper_path,
                    filetime::FileTime::from_system_time(bumped),
                );
            }
        });

        // ① Created 先发（任务在飞）
        let created = tokio::spawn(FileWatcher::process_event(
            FileSystemEvent::Created(path.clone()),
            Arc::clone(&file_index),
        ));
        // 给 Created 任务时间进入探测循环（首个轮询 20ms 内完成）
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(
            !created.is_finished(),
            "Created task must still be in flight"
        );

        // ② 立刻处理 Deleted：索引此刻为空，remove_file 落空（Ok(false)）
        FileWatcher::process_event(
            FileSystemEvent::Deleted(path.clone()),
            Arc::clone(&file_index),
        )
        .await;
        assert_eq!(file_index.get_file_count().await, 0);

        // ③ 磁盘文件消失（Deleted 事件的真实前因），放行 Created
        bumper.abort();
        std::fs::remove_file(&path).expect("remove file from disk");

        // ④ 等 Created 任务完成：探测对已消失文件超时 → best-effort
        // add_file 返回 Err → 无条目
        created.await.expect("created task must complete");

        assert_eq!(
            file_index.get_file_count().await,
            0,
            "no ghost entry may survive the out-of-order Deleted"
        );
        let files = file_index.get_files().await;
        assert!(files.iter().all(|f| f.path != path));
    }

    #[tokio::test]
    async fn created_ghost_check_inside_write_lock_blocks_vanished_file() {
        // S1 不变量钉住（对"无任何幽灵防御"的历史基线必红；对上一版
        // "提交后复查"实现的最终态同为绿——钉的是不变量而非防御位置）：读锁守卫把 Created 任务
        // 确定性卡在 add_file 的写锁获取处（位于归属校验、元数据读取与
        // EXIF 解析之后、提交之前），此刻删除磁盘文件。放行后写锁段
        // 最顶部的提交前存在性检查（try_exists）必须发现文件已消失并
        // 放弃提交——"Deleted 在提交前送达"交错里幽灵条目在进入索引
        // 之前即被拦截，无需提交后复查+撤销。
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let file_index = make_file_index(temp_dir.path());
        let path = write_image(temp_dir.path(), "ghost-window.jpg");

        // 持读锁：add_file 前段的归属校验与廉价查重（各取读锁）不受
        // 影响，任务会推进过元数据读取与 EXIF 解析，然后卡在
        // index.write() 提交点
        let guard = file_index.test_hold_index_read().await;

        let created = tokio::spawn(FileWatcher::process_event(
            FileSystemEvent::Created(path.clone()),
            Arc::clone(&file_index),
        ));

        // 越过 wait_for_file_ready（稳定窗口 200ms）与元数据/EXIF
        //（本地临时目录毫秒级）。写锁被持有 ⇒ 任务不可能经任何路径完成
        //（索引为空、文件在盘、扩展名受支持），is_finished 断言免于时序
        // 假设；唯一的时序要求是删除文件前任务已越过元数据读取
        //（800ms ≫ ~220ms 探测窗口，见上注）
        tokio::time::sleep(Duration::from_millis(800)).await;
        assert!(
            !created.is_finished(),
            "Created task must be parked at the index write lock (commit point)"
        );

        // 磁盘文件在写锁段执行前消失（Deleted 事件的真实前因）
        std::fs::remove_file(&path).expect("remove file from disk");

        // 放行：写锁段顶部的 try_exists 必须拦截提交
        drop(guard);
        created.await.expect("created task must complete");

        assert_eq!(
            file_index.get_file_count().await,
            0,
            "pre-commit existence check inside the write lock must block the ghost entry"
        );
        let files = file_index.get_files().await;
        assert!(
            files.iter().all(|f| f.path != path),
            "the vanished path must not remain indexed"
        );
    }

    #[tokio::test]
    async fn process_created_for_missing_file_times_out_but_attempts_best_effort_index() {
        // 文件始终不落地 → wait_for_file_ready 在 FILE_READY_TIMEOUT_SECS（5s）后放弃，
        // 超时分支不再丢弃事件而是 best-effort 调 add_file；add_file 因 metadata Err
        // 返回 Err（warn 路径），索引保持干净——无 panic、无 ghost 条目（~5s）
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let file_index = make_file_index(temp_dir.path());
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
        let file_index = make_file_index(temp_dir.path());
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

        FileWatcher::process_event(
            FileSystemEvent::Created(path.clone()),
            Arc::clone(&file_index),
        )
        .await;

        assert_eq!(
            file_index.get_file_count().await,
            1,
            "timeout must degrade to best-effort add_file, not skip the event"
        );
        let files = file_index.get_files().await;
        assert_eq!(
            files[0].path, path,
            "degraded index must contain the slow file"
        );

        // 收尾后台拨动任务，避免 tempdir 清理与句柄竞争
        bumper.abort();
    }

    // ---- 拉伸项（S6）----

    #[tokio::test]
    async fn event_loop_saturation_indices_all_events_beyond_permit_limit() {
        // S6①：并发上限 INDEX_EVENT_CONCURRENCY=12，投递 15 个重事件
        //（每个都经历 ≥200ms 的稳定性探测窗口，非零处理时长），断言
        // 许可饱和排队下无一丢失、最终全部入索引。
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let file_index = make_file_index(temp_dir.path());
        let (tx, rx) = channel::<FileSystemEvent>(64);
        let needs_rescan = Arc::new(AtomicBool::new(false));

        tokio::spawn(FileWatcher::run_event_loop(
            rx,
            Arc::clone(&file_index),
            needs_rescan,
        ));

        let mut paths = Vec::new();
        for i in 0..15 {
            let path = write_image(temp_dir.path(), &format!("sat_{:02}.jpg", i));
            tx.send(FileSystemEvent::Created(path.clone()))
                .await
                .unwrap();
            paths.push(path);
        }
        drop(tx);

        wait_for_count(&file_index, 15).await;
        let files = file_index.get_files().await;
        for path in &paths {
            assert!(
                files.iter().any(|f| f.path == *path),
                "{:?} must survive semaphore saturation and be indexed",
                path
            );
        }
    }

    #[test]
    fn channel_full_drops_event_with_warning_without_panic() {
        // S6②：notify 回调线程的 try_send 在通道满时必须走 warn 分支
        // 丢弃事件，不得 panic——容量 2 的通道灌入 5 个 Created 事件，
        // 只有前 2 个能入队。丢弃必须同时置位 needs_rescan（B2 收敛契约）。
        let (tx, mut rx) = channel::<FileSystemEvent>(2);
        let needs_rescan = AtomicBool::new(false);
        for _ in 0..5 {
            let event = notify::Event {
                kind: notify::event::EventKind::Create(CreateKind::File),
                paths: vec![PathBuf::from(r"C:\photos\flood.jpg")],
                ..Default::default()
            };
            FileWatcher::handle_notify_event(event, &tx, &needs_rescan);
        }
        drop(tx);

        let mut received = 0;
        while rx.try_recv().is_ok() {
            received += 1;
        }
        assert_eq!(received, 2, "only the first two events fit the channel");
        assert!(
            needs_rescan.load(Ordering::SeqCst),
            "overflow drops must set the needs_rescan flag"
        );
    }

    // ---- 丢弃事件收敛（B2）----

    #[tokio::test]
    async fn channel_overflow_drops_converge_via_merged_rescan_on_drain() {
        // B2：溢出丢弃的事件必须收敛——丢弃置 needs_rescan，事件循环在
        // 通道排空（try_recv 返回 Empty）后 check-and-swap 触发一次合并
        // 重扫。孤儿文件真实存在于磁盘但事件被丢弃，只有合并重扫能把它
        // 补进索引；修复前（Windows 会话无任何后续重扫触发点）它会一直
        // 缺失直到应用重启。
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let file_index = make_file_index(temp_dir.path());

        let a = write_image(temp_dir.path(), "a.jpg");
        let b = write_image(temp_dir.path(), "b.jpg");
        let orphan = write_image(temp_dir.path(), "orphan.jpg");

        let (tx, rx) = channel::<FileSystemEvent>(2);
        let needs_rescan = Arc::new(AtomicBool::new(false));

        // 占满容量 2（事件循环尚未启动，无消费者）；随后 3 次投递必然
        // 溢出丢弃（fake 路径不落盘，重扫也不会索引它们）
        tx.try_send(FileSystemEvent::Created(a)).expect("fill slot 1");
        tx.try_send(FileSystemEvent::Created(b)).expect("fill slot 2");
        for _ in 0..3 {
            let event = notify::Event {
                kind: notify::event::EventKind::Create(CreateKind::File),
                paths: vec![PathBuf::from(r"C:\photos\flood.jpg")],
                ..Default::default()
            };
            FileWatcher::handle_notify_event(event, &tx, &needs_rescan);
        }
        assert!(
            needs_rescan.load(Ordering::SeqCst),
            "overflow drops must set the needs_rescan flag"
        );

        tokio::spawn(FileWatcher::run_event_loop(
            rx,
            Arc::clone(&file_index),
            Arc::clone(&needs_rescan),
        ));

        // a/b 经事件入索引；排空后的合并重扫必须把从未投递成功的孤儿补进来
        wait_for_count(&file_index, 3).await;
        let files = file_index.get_files().await;
        assert!(
            files.iter().any(|f| f.path == orphan),
            "merged rescan on drain must index the orphan file"
        );
        assert_eq!(
            file_index.test_scan_call_count(),
            1,
            "a single overflow episode must converge with exactly one merged rescan"
        );

        // 第二轮溢出：flag 被重新置位 → 下一轮排空再次重扫（swap 守卫不
        // 吞新置位）。current_thread 运行时内同步灌入不会被消费任务交错，
        // 排空后通道为空、容量 2 ⇒ 第 3 个投递起必然再次溢出
        let c = write_image(temp_dir.path(), "c.jpg");
        let d = write_image(temp_dir.path(), "d.jpg");
        tx.try_send(FileSystemEvent::Created(c)).expect("refill slot 1");
        tx.try_send(FileSystemEvent::Created(d)).expect("refill slot 2");
        for _ in 0..3 {
            let event = notify::Event {
                kind: notify::event::EventKind::Create(CreateKind::File),
                paths: vec![PathBuf::from(r"C:\photos\flood-2.jpg")],
                ..Default::default()
            };
            FileWatcher::handle_notify_event(event, &tx, &needs_rescan);
        }
        assert!(
            needs_rescan.load(Ordering::SeqCst),
            "second overflow must set the flag again"
        );
        drop(tx);

        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        loop {
            if file_index.test_scan_call_count() >= 2 {
                break;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "timed out waiting for the second merged rescan (scans: {})",
                file_index.test_scan_call_count()
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    // ---- 生命周期 ----

    #[tokio::test]
    async fn watcher_start_is_idempotent_and_stop_is_idempotent() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let file_index = make_file_index(temp_dir.path());

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
