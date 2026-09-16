// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::constants::FILE_READY_TIMEOUT_SECS;
use crate::file_index::FileIndexService;
use crate::ftp::stats::StatsActor;
use crate::utils::wait_for_file_ready;
use dashmap::DashSet;
use libunftp::notification::{DataEvent, DataListener, EventMeta, PresenceEvent, PresenceListener};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tracing::{info, warn};

/// 上传文件的自动处理分类
///
/// 决定 Put 事件是否进入文件索引 / AI 修图 / 自动调色管线。
/// 从原先内联在事件处理中的 `is_raw || is_supported_image` 判定抽取为纯函数，
/// 便于单测钉住过滤语义（行为不变）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UploadedFileKind {
    /// RAW 文件：索引 + AI 修图 + 自动调色
    RawImage,
    /// 非 RAW 图片（JPEG/HEIF 系列）：索引 + AI 修图（不做自动调色）
    Image,
    /// 非图片文件：跳过自动处理管线
    Other,
}

/// 对上传/删除路径做图片格式分类（纯函数）
fn classify_uploaded_file(path: &std::path::Path) -> UploadedFileKind {
    if crate::image_utils::is_raw_file(path) {
        UploadedFileKind::RawImage
    } else if crate::image_utils::is_supported_image(path) {
        UploadedFileKind::Image
    } else {
        UploadedFileKind::Other
    }
}

/// Put 事件的后处理管线：就绪等待 → 索引 → AI 修图/调色/自动打开钩子。
///
/// 超时降级：`wait_for_file_ready` 超时时仍调用 `add_file` 尽力索引
/// （文件可能真实存在，仅写入极慢；陈旧/半写条目由 add_file 的 EXIF
/// 回填机制善后），但跳过后续钩子——AI 修图/调色/自动打开假定文件已
/// 完整写入，对半写文件执行只会产出废图。
///
/// Generic over the runtime so tests can drive it with a mock
/// `AppHandle<MockRuntime>` (production always passes `AppHandle<Wry>`);
/// same pattern as `ai_edit::service::worker_loop`.
async fn run_put_pipeline<R: tauri::Runtime>(
    handle: &tauri::AppHandle<R>,
    full_path: std::path::PathBuf,
    is_raw: bool,
) {
    if !wait_for_file_ready(&full_path, Duration::from_secs(FILE_READY_TIMEOUT_SECS)).await {
        tracing::warn!(
            "File not ready after timeout, indexing best-effort, skipping post-processing hooks: {:?}",
            full_path
        );
        // 尽力索引（成功路径的索引步骤），然后 return——不进入钩子
        if let Some(file_index) = handle.try_state::<Arc<FileIndexService>>() {
            if let Err(e) = file_index.add_file(full_path.clone()).await {
                tracing::warn!("Failed to add file to index: {}", e);
            }
            // 延迟重试：纯 watcher/单事件场景下降级索引后没有第二次触发
            // 点（此 Put 事件不会重发），30s 后再调一次 add_file，让 EXIF
            // 回填机制有机会善后半写/陈旧条目；重试失败仅 warn。
            //（try_state 返回的 State 借用 handle，spawn 需要 'static，
            // 先克隆出 Arc）
            let retry_index: Arc<FileIndexService> = file_index.inner().clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_secs(30)).await;
                if let Err(e) = retry_index.add_file(full_path).await {
                    tracing::warn!("Delayed re-index retry failed: {}", e);
                }
            });
        }
        return;
    }

    // File indexing
    if let Some(file_index) = handle.try_state::<Arc<FileIndexService>>() {
        if let Err(e) = file_index.add_file(full_path.clone()).await {
            tracing::warn!("Failed to add file to index: {}", e);
        }
    }

    // AI edit (all platforms)
    let ai_edit: tauri::State<'_, crate::ai_edit::AiEditService> = handle.state();
    ai_edit.on_file_uploaded(full_path.clone()).await;

    // Auto color grading (RAW files only)
    if is_raw {
        let color_grading: tauri::State<'_, std::sync::Arc<crate::color_grading::ColorGradingService>> = handle.state();
        color_grading.on_file_uploaded(full_path.clone()).await;
    }

    // Auto-open (Windows only)
    #[cfg(target_os = "windows")]
    {
        let auto_open: tauri::State<'_, crate::auto_open::AutoOpenService> = handle.state();
        if let Err(e) = auto_open.on_file_uploaded(full_path).await {
            tracing::error!("Failed to auto open image: {}", e);
        }
    }
    #[cfg(not(target_os = "windows"))]
    let _ = &full_path; // suppress unused warning
}

/// 数据事件监听器（上传、下载等）
#[derive(Debug, Clone)]
pub struct FtpDataListener {
    stats: StatsActor,
    save_path: Arc<std::path::PathBuf>,
    app_handle: Option<AppHandle>,
}

impl FtpDataListener {
    pub fn new(stats: StatsActor, save_path: std::path::PathBuf, app_handle: Option<AppHandle>) -> Self {
        Self { stats, save_path: Arc::new(save_path), app_handle }
    }
}

impl DataListener for FtpDataListener {
    fn receive_data_event<'life0, 'async_trait>(
        &'life0 self,
        event: DataEvent,
        _meta: EventMeta,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
    {
        let stats = self.stats.clone();
        let save_path = self.save_path.clone();
        let app_handle = self.app_handle.clone();
        Box::pin(async move {
            match event {
                DataEvent::Put { path, bytes } => {
                    // 上传统计与 "File uploaded" 日志由 StatsActor 统一记录
                    stats.record_upload(path.clone(), bytes).await;

                    let kind = classify_uploaded_file(std::path::Path::new(&path));

                    if kind != UploadedFileKind::Other {
                        let is_raw = kind == UploadedFileKind::RawImage;
                        if let Some(handle) = app_handle.as_ref() {
                            let full_path = save_path.join(&path);
                            let handle_clone = handle.clone();
                            tokio::spawn(async move {
                                run_put_pipeline(&handle_clone, full_path, is_raw).await;
                            });
                        }
                    } else {
                        info!(file = %path, "Non-image file uploaded, skipping auto-preview");
                    }
                }
                DataEvent::Got { path, bytes } => {
                    info!(file = %path, size = bytes, "File downloaded");
                }
                DataEvent::Deleted { path } => {
                    info!(file = %path, "File deleted");

                    let is_image =
                        classify_uploaded_file(std::path::Path::new(&path)) != UploadedFileKind::Other;

                    // 从文件索引中移除
                    if let Some(handle) = app_handle.as_ref() {
                        let full_path = save_path.join(&path);
                        let handle_clone = handle.clone();
                        tokio::spawn(async move {
                            if let Some(file_index) = handle_clone.try_state::<Arc<FileIndexService>>() {
                                if let Err(e) = file_index.remove_file(&full_path).await {
                                    tracing::warn!("Failed to remove file from index: {}", e);
                                }
                            }
                        });

                        if is_image {
                            if let Err(err) = handle.emit("media-library-refresh-requested", ()) {
                                warn!(error = %err, file = %path, "Failed to emit media refresh event after delete");
                            }
                        }
                    }
                }
                DataEvent::MadeDir { path } => {
                    info!(dir = %path, "Directory created");
                }
                DataEvent::RemovedDir { path } => {
                    info!(dir = %path, "Directory removed");
                }
                DataEvent::Renamed { from, to } => {
                    info!(from = %from, to = %to, "File renamed");
                }
            }
        })
    }
}

/// 在线状态监听器（登录、登出）
#[derive(Debug, Clone)]
pub struct FtpPresenceListener {
    stats: StatsActor,
    sessions: Arc<DashSet<String>>,
}

impl FtpPresenceListener {
    pub fn new(stats: StatsActor, sessions: Arc<DashSet<String>>) -> Self {
        Self { stats, sessions }
    }
}

impl PresenceListener for FtpPresenceListener {
    fn receive_presence_event<'life0, 'async_trait>(
        &'life0 self,
        event: PresenceEvent,
        meta: EventMeta,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'async_trait>>
    where
        'life0: 'async_trait,
    {
        let sessions = self.sessions.clone();
        let stats = self.stats.clone();

        Box::pin(async move {
            match event {
                PresenceEvent::LoggedIn => {
                    let is_new = sessions.insert(meta.trace_id.clone());
                    let count = sessions.len() as u64;

                    if is_new {
                        info!(
                            username = %meta.username,
                            trace_id = %meta.trace_id,
                            total_connections = count,
                            "User logged in"
                        );
                    } else {
                        warn!(
                            username = %meta.username,
                            trace_id = %meta.trace_id,
                            "Duplicate LoggedIn event received"
                        );
                    }

                    stats.update_connection_count(count).await;
                }
                PresenceEvent::LoggedOut => {
                    let existed = sessions.remove(&meta.trace_id).is_some();
                    let count = sessions.len() as u64;

                    if existed {
                        info!(
                            username = %meta.username,
                            trace_id = %meta.trace_id,
                            total_connections = count,
                            "User logged out"
                        );
                    } else {
                        warn!(
                            username = %meta.username,
                            trace_id = %meta.trace_id,
                            "LoggedOut for unknown session"
                        );
                    }

                    stats.update_connection_count(count).await;
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event_meta(trace_id: &str) -> EventMeta {
        EventMeta {
            username: "camera".to_string(),
            trace_id: trace_id.to_string(),
            sequence_number: 1,
        }
    }

    /// 轮询直到条件满足（与 color_grading::service 测试的 wait_until 同款，支持异步探针）
    async fn wait_until<F, Fut, T>(timeout: Duration, mut probe: F) -> T
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = Option<T>>,
    {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if let Some(value) = probe().await {
                return value;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "timed out after {:?} waiting for condition",
                timeout
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    // ---- Put 事件：上传统计在图片过滤之前无条件记录 ----
    //
    // 注：Put 的完整成功管线（就绪等待 → 索引 → AI/调色/自动打开钩子）
    // 需要从 AppHandle 解析 AiEditService / AutoOpenService /
    // ColorGradingService 等具体 Wry 句柄类型，MockRuntime 无法构造这些
    // 服务，因此成功路径无法在单测中确定性驱动（驱动它会因
    // state::<AiEditService>() 缺失而 panic）。管线已抽取为
    // run_put_pipeline<R: Runtime>，其超时降级路径在索引后即 return、
    // 不触达任何钩子，可用 MockRuntime 直接驱动（见下方测试）。

    #[tokio::test]
    async fn put_pipeline_timeout_degrades_to_best_effort_index_and_skips_hooks() {
        // 等价逻辑层测试（对齐 ai_edit/color_grading 的 MockRuntime 模式）：
        // mock app 只管理 FileIndexService。mtime 全程持续拨动使
        // wait_for_file_ready 在 FILE_READY_TIMEOUT_SECS（5s）后必然超时，
        // 文件真实存在 → 降级路径必须仍调用 add_file（索引 1 条）。
        // 测试跑完不 panic 本身即证明超时路径在索引后 return——若误入
        // 成功路径，state::<AiEditService>() 会因服务缺失而 panic（~5s）。
        let app = tauri::test::mock_app();
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let save_path = temp_dir.path().join("images");
        std::fs::create_dir_all(&save_path).expect("create save path");

        let config_service = Arc::new(crate::config_service::ConfigService::new_with_path(
            temp_dir.path().join("config.json"),
        ));
        config_service
            .mutate_and_persist(|c| {
                c.save_path = save_path.clone();
            })
            .expect("persist config");
        let file_index = Arc::new(crate::file_index::FileIndexService::new(config_service));
        app.manage(Arc::clone(&file_index));

        let path = save_path.join("slow-write.jpg");
        std::fs::write(&path, b"jpeg-bytes").expect("write file");
        // 后台持续拨动 mtime（len 不变），让 (len, mtime) 签名在整个 5s
        // 探测窗口内永不稳定 → wait_for_file_ready 必然走超时分支
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

        run_put_pipeline(app.handle(), path.clone(), false).await;
        bumper.abort();

        assert_eq!(
            file_index.get_file_count().await, 1,
            "timeout must degrade to best-effort add_file, not skip the event"
        );
        let files = file_index.get_files().await;
        assert_eq!(files[0].path, path, "degraded index must contain the slow file");
    }

    #[tokio::test]
    async fn put_events_record_uploads_in_stats_regardless_of_file_kind() {
        let (stats, worker) = StatsActor::with_event_bus(None);
        let stats_probe = stats.clone();
        let _worker = tokio::spawn(worker.run());

        // 无 AppHandle：上传管线整体跳过，但统计仍必须记录
        let listener =
            FtpDataListener::new(stats, std::path::PathBuf::from("/tmp/cameraftp"), None);

        listener
            .receive_data_event(
                DataEvent::Put { path: "photo.jpg".to_string(), bytes: 2048 },
                event_meta("t-put-1"),
            )
            .await;
        listener
            .receive_data_event(
                DataEvent::Put { path: "notes.txt".to_string(), bytes: 32 },
                event_meta("t-put-2"),
            )
            .await;

        // 图片与非图片上传都计数：统计发生在过滤之前
        let snapshot = wait_until(Duration::from_secs(5), || {
            let stats_probe = stats_probe.clone();
            async move {
                let stats = stats_probe.get_stats_direct().await;
                (stats.total_uploads == 2).then_some(stats)
            }
        })
        .await;

        assert_eq!(snapshot.total_uploads, 2);
        assert_eq!(snapshot.total_bytes_received, 2048 + 32);
        assert_eq!(snapshot.last_uploaded_file.as_deref(), Some("notes.txt"));
    }

    #[test]
    fn classify_uploaded_file_separates_raw_images_and_non_images() {
        use std::path::Path;

        // RAW 扩展名（大小写不敏感）→ RawImage
        for name in ["a.nef", "b.CR3", "c.dng", "d.Rw2", "e.x3f"] {
            assert_eq!(
                classify_uploaded_file(Path::new(name)),
                UploadedFileKind::RawImage,
                "{} must classify as RawImage",
                name
            );
        }

        // 非 RAW 的受支持图片 → Image
        for name in ["a.jpg", "b.JPEG", "c.heic", "d.hif", "e.heif"] {
            assert_eq!(
                classify_uploaded_file(Path::new(name)),
                UploadedFileKind::Image,
                "{} must classify as Image",
                name
            );
        }

        // 非图片 → Other（包括无扩展名与 ".jpg" 这类隐藏文件名——扩展名为 None）
        for name in ["a.txt", "b.mp4", "no-extension", ".jpg"] {
            assert_eq!(
                classify_uploaded_file(Path::new(name)),
                UploadedFileKind::Other,
                "{} must classify as Other",
                name
            );
        }
    }

    #[tokio::test]
    async fn non_put_data_events_complete_without_touching_upload_stats() {
        let (stats, worker) = StatsActor::with_event_bus(None);
        let stats_probe = stats.clone();
        let _worker = tokio::spawn(worker.run());

        let listener =
            FtpDataListener::new(stats, std::path::PathBuf::from("/tmp/cameraftp"), None);

        // Got/Deleted/MadeDir/RemovedDir/Renamed 都不产生上传统计
        let events = [
            DataEvent::Got { path: "photo.jpg".to_string(), bytes: 10 },
            DataEvent::Deleted { path: "photo.jpg".to_string() },
            DataEvent::MadeDir { path: "subdir".to_string() },
            DataEvent::RemovedDir { path: "subdir".to_string() },
            DataEvent::Renamed {
                from: "a.jpg".to_string(),
                to: "b.jpg".to_string(),
            },
        ];
        for event in events {
            listener.receive_data_event(event, event_meta("t-other")).await;
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(
            stats_probe.get_stats_direct().await,
            crate::ftp::types::ServerStats::default()
        );
    }

    // ---- Presence 事件：会话去重 + 连接数同步 ----

    async fn connection_count_becomes(
        stats: &StatsActor,
        expected: u64,
    ) -> crate::ftp::types::ServerStats {
        wait_until(Duration::from_secs(5), || {
            let stats = stats.clone();
            async move {
                let stats = stats.get_stats_direct().await;
                (stats.active_connections == expected).then_some(stats)
            }
        })
        .await
    }

    #[tokio::test]
    async fn presence_events_update_connection_count_with_session_dedup() {
        let (stats, worker) = StatsActor::with_event_bus(None);
        let stats_probe = stats.clone();
        let _worker = tokio::spawn(worker.run());

        let sessions: Arc<DashSet<String>> = Arc::new(DashSet::new());
        let listener = FtpPresenceListener::new(stats, Arc::clone(&sessions));

        // 第一个会话登录 → 连接数 1，会话入集合
        listener
            .receive_presence_event(PresenceEvent::LoggedIn, event_meta("trace-1"))
            .await;
        assert_eq!(
            connection_count_becomes(&stats_probe, 1).await.active_connections,
            1
        );
        assert!(sessions.contains("trace-1"));

        // 第二个会话登录 → 连接数 2
        listener
            .receive_presence_event(PresenceEvent::LoggedIn, event_meta("trace-2"))
            .await;
        connection_count_becomes(&stats_probe, 2).await;
        assert_eq!(sessions.len(), 2);

        // 同一 trace_id 重复 LoggedIn → 去重：连接数不涨
        listener
            .receive_presence_event(PresenceEvent::LoggedIn, event_meta("trace-1"))
            .await;
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(
            stats_probe.get_stats_direct().await.active_connections, 2,
            "duplicate LoggedIn for a known session must not bump the count"
        );
        assert_eq!(sessions.len(), 2);

        // 登出已知会话 → 连接数 1，会话出集合
        listener
            .receive_presence_event(PresenceEvent::LoggedOut, event_meta("trace-1"))
            .await;
        connection_count_becomes(&stats_probe, 1).await;
        assert!(!sessions.contains("trace-1"));

        // 未知会话 LoggedOut → 计数不变、不 panic
        listener
            .receive_presence_event(PresenceEvent::LoggedOut, event_meta("ghost"))
            .await;
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(stats_probe.get_stats_direct().await.active_connections, 1);
    }
}
