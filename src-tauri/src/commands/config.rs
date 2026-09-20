// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use serde::Deserialize;
use tauri::{command, AppHandle, Manager, State};
use tracing::instrument;

use crate::auto_open::AutoOpenService;
use crate::commands::FtpServerState;
use crate::config::{AppConfig, PreviewWindowConfig};
use crate::config_service::ConfigService;
use crate::crypto;
use crate::error::AppError;
use crate::file_index::FileIndexService;
use crate::ftp::types::FtpServerSlot;
use std::sync::Arc;

async fn save_auth_config_with_service(
    config_service: &ConfigService,
    anonymous: bool,
    username: String,
    password: String,
) -> Result<(), AppError> {
    use crate::config::AuthConfig;

    // Argon2id(m=64MB,t=3,p=4) 是重 CPU 计算：放在 blocking 池执行，
    // 不占用 tokio worker 线程（与 FTP 认证路径 ftp/server.rs 一致）。
    let password_hash = if anonymous || password.is_empty() {
        String::new()
    } else {
        tokio::task::spawn_blocking(move || crypto::hash_password(password).hash)
            .await
            .map_err(|e| AppError::Other(format!("hash task failed: {}", e)))?
    };

    config_service
        .mutate_and_persist_async(move |config| {
            config.advanced_connection.auth = AuthConfig {
                anonymous,
                username,
                password_hash,
            };
        })
        .await?;

    tracing::info!("Auth config saved with Argon2id hash");
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewWindowConfigPatch {
    pub enabled: Option<bool>,
    pub method: Option<crate::config::ImageOpenMethod>,
    pub custom_path: Option<Option<String>>,
    pub auto_bring_to_front: Option<bool>,
}

impl PreviewWindowConfigPatch {
    fn apply_to(self, mut current: PreviewWindowConfig) -> PreviewWindowConfig {
        if let Some(enabled) = self.enabled {
            current.enabled = enabled;
        }
        if let Some(method) = self.method {
            current.method = method;
        }
        if let Some(custom_path) = self.custom_path {
            current.custom_path = custom_path;
        }
        if let Some(auto_bring_to_front) = self.auto_bring_to_front {
            current.auto_bring_to_front = auto_bring_to_front;
        }
        current
    }
}

fn merge_backend_owned_fields(mut incoming: AppConfig, current: &AppConfig) -> AppConfig {
    incoming.preview_config = current.preview_config.clone();
    incoming
}

async fn update_preview_config_with_service(
    config_service: &ConfigService,
    patch: PreviewWindowConfigPatch,
) -> Result<PreviewWindowConfig, AppError> {
    config_service
        .mutate_and_persist_async(move |app_config| {
            let current = app_config.preview_config.clone().unwrap_or_default();
            let merged = patch.apply_to(current);
            app_config.preview_config = Some(merged.clone());
            merged
        })
        .await
}

#[command]
#[instrument(skip(config_service))]
pub fn load_config(config_service: State<'_, Arc<ConfigService>>) -> AppConfig {
    config_service.inner().get_or_default()
}

/// save_path 变更守卫：FTP 服务器活动（Starting/Running）期间拒绝改路径。
///
/// FTP 监听器的根目录在启动时固定；若放行运行中改路径，索引会切到新根
/// 目录，而上传仍写入旧根目录——文件落盘却在图库中不可见（唯一痕迹是
/// add_file 的越界路径拒绝日志）。UI 侧同步禁用目录选择器，本守卫是
/// 后端兜底（托盘/并发窗口等绕过 UI 的写者同样被拦截）。
///
/// 返回持锁守卫（收窄方案 A）：save_path 实际变化且槽位为 None 时返回
/// `Some(槽位锁守卫)`——调用方（save_config）把「落盘 + 内存换入」
/// 关进同一段临界区，与 start_server 的认领（claim_start_slot，同一把
/// 锁）互斥，封死"守卫放行 → 并发 start 以旧配置认领启动 → persist
/// 才换入新配置"的 TOCTOU（服务器 root 钉死旧根，上传落旧根被
/// add_file 拒绝）。调用方在 persist 完成后即释放，绝不把
/// update_save_path（秒级扫描）纳入持锁段——那会阻塞 start/stop/托盘
/// 整段扫描时长；update_save_path 不触碰槽位锁，无锁序反转。
/// save_path 未变化时返回 None（不触碰槽位锁，高频配置写不受串行化
/// 影响）。
async fn ensure_save_path_change_allowed<'a>(
    config_service: &ConfigService,
    ftp_state: &'a FtpServerState,
    incoming: &AppConfig,
) -> Result<Option<tokio::sync::MutexGuard<'a, FtpServerSlot>>, AppError> {
    let current_save_path = config_service.get_or_default().save_path;
    if incoming.save_path == current_save_path {
        return Ok(None);
    }

    // 只看槽位状态（None 之外一律视为活动），不与服务器通信——
    // Starting 窗口内尚无运行时信息，且必须与启动认领同样保守
    let slot = ftp_state.0.lock().await;
    if !matches!(*slot, FtpServerSlot::None) {
        tracing::warn!(
            old = ?current_save_path,
            new = ?incoming.save_path,
            "Rejected save_path change while FTP server is starting or running"
        );
        return Err(AppError::Other(
            "Cannot change the save path while the FTP server is starting or running; \
             stop the FTP server first"
                .to_string(),
        ));
    }
    Ok(Some(slot))
}

#[command]
#[instrument(skip(app, config, config_service, file_index, ftp_state))]
pub async fn save_config(
    app: AppHandle,
    config: AppConfig,
    config_service: State<'_, Arc<ConfigService>>,
    file_index: State<'_, Arc<FileIndexService>>,
    ftp_state: State<'_, FtpServerState>,
) -> Result<(), AppError> {
    // 守卫在 save_path 实际变化时返回持槽位锁的守卫：下方
    // mutate_and_persist_async（blocking 池 + fsync 数十 ms）全程与
    // start_server 的认领互斥，杜绝"守卫放行后、内存换入前"窗口内并发
    // 启动以旧配置认领（见守卫 doc）。save_path 未变化时为 None，
    // 配置写不被串行化。
    let slot_guard =
        ensure_save_path_change_allowed(config_service.inner(), ftp_state.inner(), &config).await?;

    let (old_save_path, new_save_path) = config_service
        .mutate_and_persist_async(move |current| {
            let old_save_path = current.save_path.clone();
            *current = merge_backend_owned_fields(config, current);
            // 新路径从 mutate 后的状态直接携带，避免落盘后再 get() 二次读取
            let new_save_path = current.save_path.clone();
            (old_save_path, new_save_path)
        })
        .await?;

    // persist 已完成（mutate_and_persist_async 的返回点即内存换入点）：
    // 立刻释放槽位锁。后续 update_save_path 含秒级扫描，绝不放回持锁段
    // ——否则 start/stop/托盘会被阻塞整个扫描时长。
    drop(slot_guard);

    tracing::info!("Configuration saved successfully");

    if old_save_path != new_save_path {
        tracing::info!(
            "save_path changed from {:?} to {:?}, triggering rescan",
            old_save_path,
            new_save_path
        );
        // 先扩展 asset protocol scope（幂等且廉价）：必须先于
        // update_save_path 执行——索引切换失败时本会话的 scope 仍已
        // 就绪，预览窗口不因切换失败而缺失新目录（直到重启）
        if let Err(e) = app
            .asset_protocol_scope()
            .allow_directory(&new_save_path, true)
        {
            tracing::warn!(error = %e, "Failed to extend asset protocol scope for new save_path");
        }
        // 索引切换失败不得让 save_config 返回 Err：config 已成功落盘，
        // 前端会把 Err 当"保存失败"处理（toast + 不更新 state.config），
        // 与磁盘上已生效的新配置分叉。失败仅记日志——索引滞留旧根由
        // get_latest_file 的空索引回退扫描与重启自愈，不向前端伪造
        // 保存失败。
        if let Err(e) = Arc::clone(&file_index)
            .update_save_path(new_save_path.clone())
            .await
        {
            tracing::error!(
                error = %e,
                "Failed to switch file index to new save_path after config persisted; \
                 the persisted config remains authoritative"
            );
        }
    }

    Ok(())
}

/// 保存认证配置（使用 Argon2id 哈希密码）
#[command]
#[instrument(skip(config_service, password))]
pub async fn save_auth_config(
    config_service: State<'_, Arc<ConfigService>>,
    anonymous: bool,
    username: String,
    password: String,
) -> Result<(), AppError> {
    // Argon2 哈希在 helper 内部走 spawn_blocking、落盘走
    // mutate_and_persist_async（blocking 池），命令层无需再包裹
    // spawn_blocking——直接 await 即可。
    save_auth_config_with_service(config_service.inner(), anonymous, username, password).await
}

/// 选择保存目录
#[command]
pub async fn select_save_directory(app: AppHandle) -> Result<Option<String>, String> {
    crate::platform::get_platform()
        .select_save_directory(&app)
        .await
}

// ============================================================================
// 自动预览配置命令（Windows）
// ============================================================================

#[command]
pub async fn update_preview_config(
    auto_open: State<'_, AutoOpenService>,
    config_service: State<'_, Arc<ConfigService>>,
    patch: PreviewWindowConfigPatch,
) -> Result<PreviewWindowConfig, AppError> {
    let persisted =
        update_preview_config_with_service(config_service.inner().as_ref(), patch).await?;
    auto_open.broadcast_config_changed(persisted.clone()).await;
    Ok(persisted)
}

/// 手动打开预览窗口（遵循用户配置的打开方式）
#[command]
pub async fn open_preview_window(app: AppHandle, file_path: String) -> Result<(), AppError> {
    let path = std::path::PathBuf::from(&file_path);

    // 先在 FileIndexService 中查找并设置索引
    let file_index = app.state::<Arc<FileIndexService>>();
    if let Some(index) = file_index.find_file_index(&path).await {
        file_index.navigate_to(index).await?;
    }

    // 使用 AutoOpenService 来处理，它会根据配置决定打开方式
    let auto_open = app.state::<AutoOpenService>();
    auto_open.open_image(&path).await
}

/// 选择可执行文件（用于自定义打开程序）
#[command]
#[cfg_attr(target_os = "android", allow(unused_variables))]
pub async fn select_executable_file(app: AppHandle) -> Result<Option<String>, AppError> {
    #[cfg(target_os = "windows")]
    {
        use tauri_plugin_dialog::DialogExt;

        let file_path: Option<tauri_plugin_dialog::FilePath> =
            tokio::task::spawn_blocking(move || {
                app.dialog()
                    .file()
                    .set_title("选择程序")
                    .add_filter("可执行文件", &["exe"])
                    .blocking_pick_file()
            })
            .await
            .map_err(|e| AppError::Other(format!("Task failed: {}", e)))?;

        Ok(file_path.and_then(|p| p.as_path().map(|path| path.to_string_lossy().to_string())))
    }

    #[cfg(target_os = "android")]
    {
        Ok(None)
    }
}

/// 打开外部链接
#[command]
pub async fn open_external_link(url: String) -> Result<(), AppError> {
    crate::platform::get_platform()
        .open_external_link(&url)
        .map_err(AppError::Other)
}

/// 打开文件夹并选中文件（Windows 资源管理器）
#[command]
pub async fn open_folder_select_file(file_path: String) -> Result<(), AppError> {
    #[cfg(target_os = "windows")]
    {
        crate::auto_open::windows::open_folder_and_select_file(&std::path::PathBuf::from(
            &file_path,
        ))
    }

    #[cfg(target_os = "android")]
    {
        let _ = file_path;
        Ok(())
    }
}

/// 打开存储目录（Windows 资源管理器）
#[command]
pub fn open_save_directory(config_service: State<'_, Arc<ConfigService>>) -> Result<(), AppError> {
    #[cfg(target_os = "windows")]
    {
        let config = config_service.inner().get_or_default();
        crate::auto_open::windows::open_directory(&config.save_path)
    }

    #[cfg(target_os = "android")]
    {
        let _ = config_service;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::config::{ImageOpenMethod, PreviewWindowConfig};
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn get_or_default_reads_service_state() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let config_path = temp_dir.path().join("config.json");
        let service = ConfigService::new_with_path(config_path);
        service.load().expect("failed to load config");

        service
            .mutate_and_persist(|config| config.port = 3777)
            .expect("failed to update config");

        let loaded = service.get_or_default();
        assert_eq!(loaded.port, 3777);
    }

    #[test]
    fn tauri_conf_csp_covers_preview_and_asset_origins() {
        // 回归：Windows 预览窗口依赖 image-preview scheme 与 asset protocol，
        // CSP 缺失任一来源会导致 <img> 加载失败（测试 CWD = src-tauri）
        let raw = std::fs::read_to_string("tauri.conf.json").expect("read tauri.conf.json");
        let conf: serde_json::Value = serde_json::from_str(&raw).expect("parse tauri.conf.json");
        let csp = conf["app"]["security"]["csp"]
            .as_str()
            .expect("csp must be a string");
        for required in [
            "http://image-preview.localhost",
            "asset:",
            "http://asset.localhost",
            "default-src 'self'",
        ] {
            assert!(csp.contains(required), "CSP must contain {:?}", required);
        }
    }

    #[tokio::test]
    async fn helper_save_auth_persists_via_service() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let config_path = temp_dir.path().join("config.json");
        let service = ConfigService::new_with_path(config_path.clone());
        service.load().expect("failed to load config");

        save_auth_config_with_service(
            &service,
            false,
            "camera-user".to_string(),
            "secret-pass".to_string(),
        )
        .await
        .expect("failed to save auth config");

        let persisted_service = ConfigService::new_with_path(config_path);
        let reloaded = persisted_service.load().expect("failed to reload config");
        let auth = reloaded.advanced_connection.auth;

        assert!(!auth.anonymous);
        assert_eq!(auth.username, "camera-user");
        assert!(!auth.password_hash.is_empty());
        assert_ne!(auth.password_hash, "secret-pass");
    }

    #[test]
    fn helper_merge_backend_owned_fields_preserves_preview_config() {
        let current = AppConfig {
            preview_config: Some(PreviewWindowConfig {
                enabled: true,
                method: ImageOpenMethod::WindowsPhotos,
                custom_path: Some("C:/Program Files/Photos/photos.exe".to_string()),
                auto_bring_to_front: true,
            }),
            ..AppConfig::default()
        };
        let incoming = AppConfig {
            preview_config: Some(PreviewWindowConfig::default()),
            ..AppConfig::default()
        };

        let merged = merge_backend_owned_fields(incoming, &current);
        let preview = merged
            .preview_config
            .expect("preview should still be present");
        assert!(matches!(preview.method, ImageOpenMethod::WindowsPhotos));
        assert!(preview.auto_bring_to_front);
    }

    #[test]
    fn helper_preview_patch_updates_only_requested_fields() {
        let current = PreviewWindowConfig {
            enabled: true,
            method: ImageOpenMethod::BuiltInPreview,
            custom_path: Some("viewer.exe".to_string()),
            auto_bring_to_front: false,
        };
        let patch = PreviewWindowConfigPatch {
            enabled: None,
            method: Some(ImageOpenMethod::SystemDefault),
            custom_path: None,
            auto_bring_to_front: Some(true),
        };

        let merged = patch.apply_to(current);
        assert!(matches!(merged.method, ImageOpenMethod::SystemDefault));
        assert_eq!(merged.custom_path, Some("viewer.exe".to_string()));
        assert!(merged.auto_bring_to_front);
    }

    #[tokio::test]
    async fn helper_update_preview_patch_merges_against_latest_persisted_config() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let config_path = temp_dir.path().join("config.json");
        let service = ConfigService::new_with_path(config_path.clone());
        service.load().expect("failed to load config");

        let updated = update_preview_config_with_service(
            &service,
            PreviewWindowConfigPatch {
                enabled: Some(false),
                method: Some(ImageOpenMethod::WindowsPhotos),
                custom_path: Some(Some("C:/Program Files/WindowsApps/photos.exe".to_string())),
                auto_bring_to_front: Some(false),
            },
        )
        .await
        .expect("failed to initialize preview config");

        assert!(!updated.enabled);
        assert!(!updated.auto_bring_to_front);

        let updated = update_preview_config_with_service(
            &service,
            PreviewWindowConfigPatch {
                enabled: Some(true),
                method: None,
                custom_path: None,
                auto_bring_to_front: Some(true),
            },
        )
        .await
        .expect("failed to update preview config");

        assert!(updated.enabled);
        assert!(updated.auto_bring_to_front);
        assert!(matches!(updated.method, ImageOpenMethod::WindowsPhotos));
        assert_eq!(
            updated.custom_path,
            Some("C:/Program Files/WindowsApps/photos.exe".to_string())
        );

        let persisted = service
            .get()
            .expect("failed to read persisted config")
            .preview_config
            .clone()
            .expect("preview config should exist");
        assert!(persisted.enabled);
        assert!(matches!(persisted.method, ImageOpenMethod::WindowsPhotos));
    }

    #[tokio::test]
    async fn helper_update_preview_patch_returns_error_when_persistence_fails() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let blocked_parent = temp_dir.path().join("blocked-parent");
        std::fs::write(&blocked_parent, "not a directory").expect("failed to create blocker file");

        let service = ConfigService::new_with_path(blocked_parent.join("config.json"));
        let result = update_preview_config_with_service(
            &service,
            PreviewWindowConfigPatch {
                enabled: Some(true),
                method: None,
                custom_path: None,
                auto_bring_to_front: None,
            },
        )
        .await;

        assert!(result.is_err());
    }

    // ---- save_path 变更守卫（B1：FTP 服务器活动期间拒绝改路径）----

    use std::path::PathBuf;

    use crate::ftp::types::FtpServerSlot;

    fn ftp_state_with_slot(slot: FtpServerSlot) -> super::super::FtpServerState {
        crate::commands::FtpServerState(std::sync::Arc::new(tokio::sync::Mutex::new(slot)))
    }

    fn guard_test_config_service(dir: &std::path::Path) -> ConfigService {
        let service = ConfigService::new_with_path(dir.join("config.json"));
        service.load().expect("failed to load config");
        service
    }

    fn incoming_with_save_path(path: &str) -> AppConfig {
        AppConfig {
            save_path: PathBuf::from(path),
            ..AppConfig::default()
        }
    }

    #[tokio::test]
    async fn save_path_guard_rejects_change_while_slot_starting_or_running() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let service = guard_test_config_service(temp_dir.path());
        let incoming = incoming_with_save_path("D:/photos/new-root");

        // Starting（启动权已认领）：视同运行中拒绝
        let state = ftp_state_with_slot(FtpServerSlot::Starting);
        let err = super::ensure_save_path_change_allowed(&service, &state, &incoming)
            .await
            .expect_err("must reject save_path change while Starting");
        assert!(
            err.to_string().to_lowercase().contains("save path"),
            "error should mention the save path, got: {}",
            err
        );

        // Running：同样拒绝（句柄仅为槽位载荷，守卫只看槽位状态，
        // 不与服务器通信——无需真正启动 Actor）
        let (handle, _actor, _stats_worker, _event_bus) = crate::ftp::create_ftp_server(None);
        let state = ftp_state_with_slot(FtpServerSlot::Running(handle));
        let err = super::ensure_save_path_change_allowed(&service, &state, &incoming)
            .await
            .expect_err("must reject save_path change while Running");
        assert!(
            err.to_string().to_lowercase().contains("save path"),
            "error should mention the save path, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn save_path_guard_allows_change_when_slot_none() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let service = guard_test_config_service(temp_dir.path());
        let incoming = incoming_with_save_path("D:/photos/new-root");

        let state = ftp_state_with_slot(FtpServerSlot::None);
        super::ensure_save_path_change_allowed(&service, &state, &incoming)
            .await
            .expect("save_path change must be allowed while server stopped");
    }

    #[tokio::test]
    async fn save_path_guard_allows_save_when_save_path_unchanged_even_while_active() {
        let temp_dir = tempdir().expect("failed to create temp dir");
        let service = guard_test_config_service(temp_dir.path());

        // 服务器启动中，但本次保存不改动 save_path（其余字段照常保存）
        let unchanged = service.get_or_default();
        let state = ftp_state_with_slot(FtpServerSlot::Starting);
        super::ensure_save_path_change_allowed(&service, &state, &unchanged)
            .await
            .expect("saves that keep save_path must pass even while server active");
    }

    #[tokio::test]
    async fn save_path_change_holds_slot_lock_until_persist_completes() {
        // #5 TOCTOU（收窄方案 A）时序钉住：守卫放行后持槽位锁跨越
        // persist 窗口——持锁期间并发认领（start_server 的
        // claim_start_slot 第一步，同一把锁）必须被挡住；守卫释放后
        // 认领才能进行（届时内存配置已换入，认领后的启动读到新配置）。
        // 注：未注入 persist 延迟钩子（ConfigService 无此设施且不在本
        // 车道域内）——互斥阻塞是逻辑必然而非时序概率，超时仅作断言
        // 载体，无 flaky 风险。
        let temp_dir = tempdir().expect("failed to create temp dir");
        let service = guard_test_config_service(temp_dir.path());
        service
            .mutate_and_persist(|config| config.save_path = PathBuf::from("D:/photos/old-root"))
            .expect("seed current save_path");
        let incoming = incoming_with_save_path("D:/photos/new-root");
        let state = ftp_state_with_slot(FtpServerSlot::None);

        let guard = super::ensure_save_path_change_allowed(&service, &state, &incoming)
            .await
            .expect("guard must pass while slot is None")
            .expect("guard must hold the slot lock when save_path changes");

        // 持锁窗口内的并发认领：模拟 claim_start_slot 的"锁内占位"
        let mut claim = tokio::spawn({
            let slot_mutex = std::sync::Arc::clone(&state.0);
            async move {
                let mut slot = slot_mutex.lock().await;
                *slot = FtpServerSlot::Starting;
            }
        });
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(100), &mut claim)
                .await
                .is_err(),
            "concurrent slot claim must be blocked while save_config holds the guard"
        );

        // persist 完成（守卫释放）后认领必须能继续
        drop(guard);
        claim.await.expect("claim task must not panic");
        assert!(matches!(*state.0.lock().await, FtpServerSlot::Starting));
    }
}
