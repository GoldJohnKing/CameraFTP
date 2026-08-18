// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::traits::PlatformService;
use super::types::{PermissionStatus, StorageInfo};
use crate::constants::ANDROID_DCIM_PATH;
use crate::ftp::types::ServerStateSnapshot;
use crate::utils::fs::is_path_writable;
use tauri::AppHandle;
use tracing::{debug, error, info};

#[cfg(target_os = "android")]
use jni::objects::{JObject, JValue};
#[cfg(target_os = "android")]
use crate::utils::jni::{
    android_context, clear_pending_exception, jni_ok, load_app_class, with_env,
};

#[cfg(target_os = "android")]
const ANDROID_SERVICE_COORDINATOR_CLASS: &str =
    "com.gjk.cameraftpcompanion.AndroidServiceStateCoordinator";
#[cfg(target_os = "android")]
const SYNC_ANDROID_SERVICE_STATE_METHOD: &str = "syncNativeServiceState";
#[cfg(target_os = "android")]
const SYNC_ANDROID_SERVICE_STATE_SIGNATURE: &str =
    "(Landroid/content/Context;ZLjava/lang/String;I)V";

// 重新导出常量（使用 crate 路径避免导入警告）
pub use crate::constants::ANDROID_DEFAULT_STORAGE_PATH as DEFAULT_STORAGE_PATH;
pub use crate::constants::ANDROID_STORAGE_DISPLAY_NAME as STORAGE_DISPLAY_NAME;

/// 检查 DCIM 目录是否可写（用于判断所有文件访问权限）
fn can_write_to_dcim() -> bool {
    let dcim_path = std::path::Path::new(ANDROID_DCIM_PATH);
    if !dcim_path.exists() {
        debug!("DCIM path does not exist");
        return false;
    }
    let writable = is_path_writable(dcim_path).unwrap_or_else(|e| {
        debug!("DCIM writable check failed: {}", e);
        false
    });
    if writable {
        debug!("All files access permission: granted (DCIM writable)");
    } else {
        debug!("All files access permission: denied (DCIM not writable)");
    }
    writable
}

/// 确保路径可写（不存在时创建）
fn ensure_path_writable(path: &str) -> bool {
    let path_buf = std::path::PathBuf::from(path);

    // 如果路径不存在，尝试创建
    if !path_buf.exists() {
        debug!("Path does not exist, attempting to create: {:?}", path_buf);
        match std::fs::create_dir_all(&path_buf) {
            Ok(_) => {
                info!("Successfully created directory: {:?}", path_buf);
            }
            Err(e) => {
                error!("Failed to create directory {:?}: {}", path_buf, e);
                return false;
            }
        }
    }

    // 确保是目录
    if !path_buf.is_dir() {
        error!("Path exists but is not a directory: {:?}", path_buf);
        return false;
    }

    // 使用共享辅助函数检查可写性
    let writable = is_path_writable(&path_buf).unwrap_or_else(|e| {
        error!("Path writable check failed for {:?}: {}", path_buf, e);
        false
    });
    if writable {
        debug!("Path is writable: {:?}", path_buf);
    } else {
        error!("Path is not writable: {:?}", path_buf);
    }
    writable
}

/// Android 平台实现
pub struct AndroidPlatform;

/// 服务状态同步的全局单调序号（乱序防护，见 sync_android_service_state）。
static LATEST_SYNC_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// 串行化服务状态同步 JNI 执行的文件级锁（见 sync_android_service_state）。
/// 锁中毒仅意味着某次同步任务 panic 过，互斥语义不受影响，照常使用。
static SYNC_EXEC_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

impl PlatformService for AndroidPlatform {
    fn name(&self) -> &'static str {
        "android"
    }

    fn setup(&self, _app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
        tracing::info!("Android platform initialized");
        Ok(())
    }

    fn get_storage_info(&self) -> StorageInfo {
        let path = DEFAULT_STORAGE_PATH;
        let path_buf = std::path::PathBuf::from(path);

        let exists = path_buf.exists();
        let writable = if exists {
            ensure_path_writable(path)
        } else {
            false
        };

        let has_all_files_access = writable || (exists && can_write_to_dcim());

        StorageInfo {
            display_name: STORAGE_DISPLAY_NAME.to_string(),
            path: path.to_string(),
            exists,
            writable,
            has_all_files_access,
        }
    }

    fn check_permission_status(&self) -> PermissionStatus {
        let has_access = can_write_to_dcim();
        PermissionStatus {
            has_all_files_access: has_access,
            needs_user_action: !has_access,
        }
    }

    fn ensure_storage_ready(&self, _app: &AppHandle) -> Result<String, String> {
        let path = DEFAULT_STORAGE_PATH;
        let path_buf = std::path::PathBuf::from(path);

        // 注意：这里刻意【不】做 ensure_path_writable 可写探测（与 Windows 实现不对称）。
        // Android 的 FTP 写入走 AndroidMediaStoreBackend（ContentResolver/MediaStore insert），
        // 不需要对保存目录的原生 fs 写权限；应用权限模型也是 READ_MEDIA_IMAGES（scoped media）
        // 而非 MANAGE_EXTERNAL_STORAGE。若在此探测 File::create，会在"目录已存在但仅有
        // 媒体权限"的正常设备上返回 EACCES，导致服务器无法启动（2026-08 回归）。
        // 仅保留"不存在则创建"语义：创建失败（如无任何存储权限）才报错。
        if !path_buf.exists() {
            std::fs::create_dir_all(&path_buf).map_err(|e| format!("无法创建存储目录: {}", e))?;
            info!("Created storage directory: {}", path);
        }

        Ok(path.to_string())
    }

    fn check_server_start_prerequisites(&self) -> super::types::ServerStartCheckResult {
        // Android 平台：前端通过 PermissionDialog 处理权限检查
        // 这里始终返回可启动，因为权限检查在前端完成
        // 前端会确保用户已授权所有文件访问权限后才允许启动服务器
        let storage_info = self.get_storage_info();
        super::types::ServerStartCheckResult {
            can_start: true,
            reason: None,
            storage_info: Some(storage_info),
        }
    }

    // Note: on_server_started/on_server_stopped use default empty implementation.
    // Direction 1 routes Android foreground-service updates through the frontend bridge.

    fn sync_android_service_state(&self, _app: &AppHandle, snapshot: &ServerStateSnapshot) {
        #[cfg(target_os = "android")]
        {
            // JNI 调用是同步阻塞的：放入 spawn_blocking 执行，
            // 避免阻塞事件处理所在的异步运行时线程。
            // 任务内自行记录错误（fire-and-forget，无需向同步 trait 调用方回传结果）。
            //
            // 阻塞线程池并发执行，完成顺序不保证。Kotlin 侧的
            // updateRunningState 在 isRunning=false→true 时会重启前台服务，
            // 若旧的 running 快照在 stop 快照之后落地，前台服务会被错误地
            // 重新拉起并滞留。因此用全局单调序号让被更新的快照跳过执行，
            // 保证最新快照总是最后生效。
            //
            // 仅靠序号检查仍存在窗口：旧任务检查通过后、JNI 完成前，新
            // 快照可能先落地，导致旧状态最终生效。故在任务内先持锁串行化
            // JNI 执行，再二次检查序号：过期则跳过；相等才执行，执行期间
            // 持锁，确保最新快照总是最后生效。
            use std::sync::atomic::Ordering;
            let seq = LATEST_SYNC_SEQ.fetch_add(1, Ordering::SeqCst) + 1;
            let snapshot = snapshot.clone();
            tokio::task::spawn_blocking(move || {
                // 先取锁串行化：后续任务必须等本次 JNI 完成后才能检查/执行。
                // （poison-tolerant，与 utils/task_worker.rs CancelGate 风格一致）
                let _guard = SYNC_EXEC_MUTEX.lock().unwrap_or_else(|p| p.into_inner());
                // 已有更新的快照排队：跳过本次同步，避免乱序覆盖。
                if seq != LATEST_SYNC_SEQ.load(Ordering::SeqCst) {
                    debug!("skipped stale sync seq={}", seq);
                    return;
                }
                if let Err(error) = sync_android_service_state(&snapshot) {
                    error!(%error, ?snapshot, "Failed to sync Android native service state");
                }
            });
        }

        info!(
            ?snapshot,
            "Syncing Android native service state from Rust events"
        );
    }

    fn get_default_storage_path(&self) -> std::path::PathBuf {
        std::path::PathBuf::from(DEFAULT_STORAGE_PATH)
    }

    // ========== 窗口与UI相关 ==========

    fn hide_main_window(&self, _app: &AppHandle) -> Result<(), String> {
        // Android 没有"窗口"概念，直接返回成功
        Ok(())
    }

    fn select_save_directory(&self, _app: &AppHandle) -> Result<Option<String>, String> {
        // Android 使用固定路径，直接返回默认路径
        Ok(Some(DEFAULT_STORAGE_PATH.to_string()))
    }
}

#[cfg(target_os = "android")]
fn sync_android_service_state(snapshot: &ServerStateSnapshot) -> Result<(), String> {
    use crate::error::AppError;

    with_env(|env| {
        let context = android_context(env)?;
        let coordinator_class =
            load_app_class(env, &context, ANDROID_SERVICE_COORDINATOR_CLASS)?;
        let stats_json = match serde_json::to_string(snapshot) {
            Ok(value) if snapshot.is_running => Some(value),
            Ok(_) => None,
            Err(e) => {
                return Err(AppError::Other(format!(
                    "Failed to serialize service snapshot: {e}"
                )))
            }
        };
        let stats_arg = match stats_json.as_deref() {
            Some(value) => JObject::from(jni_ok(
                env,
                "Failed to create stats JSON string",
                |env| env.new_string(value),
            )?),
            None => JObject::null(),
        };
        let connected_clients = i32::try_from(snapshot.connected_clients).map_err(|_| {
            AppError::Other(format!(
                "Connected client count exceeds Android JNI range: {}",
                snapshot.connected_clients
            ))
        })?;

        if let Err(e) = env.call_static_method(
            coordinator_class,
            SYNC_ANDROID_SERVICE_STATE_METHOD,
            SYNC_ANDROID_SERVICE_STATE_SIGNATURE,
            &[
                JValue::Object(&context),
                JValue::Bool(snapshot.is_running.into()),
                JValue::Object(&stats_arg),
                JValue::Int(connected_clients),
            ],
        ) {
            // 调用失败时可能残留 pending Java 异常，必须清除，
            // 否则当前线程后续所有 JNI 调用都会失败
            clear_pending_exception(env);
            return Err(AppError::Other(format!(
                "Failed to call syncNativeServiceState: {e}"
            )));
        }

        Ok(())
    })
    .map_err(|e| e.user_message())
}
