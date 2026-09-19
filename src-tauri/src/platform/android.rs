// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use async_trait::async_trait;

use super::traits::PlatformService;
use super::types::{PermissionStatus, StorageInfo};
use crate::constants::ANDROID_DCIM_PATH;
use crate::ftp::types::ServerStateSnapshot;
use crate::utils::fs::is_path_writable;
use tauri::AppHandle;
use tracing::{debug, error, info};

#[cfg(target_os = "android")]
use crate::utils::jni::{
    android_context, clear_pending_exception, jni_ok, load_app_class, with_env,
};
#[cfg(target_os = "android")]
use jni::objects::{JObject, JValue};

#[cfg(target_os = "android")]
const ANDROID_SERVICE_COORDINATOR_CLASS: &str =
    "com.gjk.cameraftpcompanion.AndroidServiceStateCoordinator";
#[cfg(target_os = "android")]
const SYNC_ANDROID_SERVICE_STATE_METHOD: &str = "syncNativeServiceState";
#[cfg(target_os = "android")]
const SYNC_ANDROID_SERVICE_STATE_SIGNATURE: &str =
    "(Landroid/content/Context;ZLjava/lang/String;I)V";
#[cfg(target_os = "android")]
const SYNC_NATIVE_PROCESSING_STATE_METHOD: &str = "syncNativeProcessingState";
#[cfg(target_os = "android")]
const SYNC_NATIVE_PROCESSING_STATE_SIGNATURE: &str = "(Landroid/content/Context;ZLjava/lang/String;)V";
#[cfg(target_os = "android")]
const SYNC_NATIVE_PROCESSING_PROGRESS_METHOD: &str = "syncNativeProcessingProgress";
#[cfg(target_os = "android")]
const SYNC_NATIVE_PROCESSING_PROGRESS_SIGNATURE: &str =
    "(Landroid/content/Context;Ljava/lang/String;)V";

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

#[async_trait]
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

    async fn select_save_directory(&self, _app: &AppHandle) -> Result<Option<String>, String> {
        // Android 使用固定路径，直接返回默认路径
        Ok(Some(DEFAULT_STORAGE_PATH.to_string()))
    }

    fn open_external_link(&self, _url: &str) -> Result<(), String> {
        // Android 平台通过 JavaScript bridge 处理外部链接
        Ok(())
    }
}

#[cfg(target_os = "android")]
fn sync_android_service_state(snapshot: &ServerStateSnapshot) -> Result<(), String> {
    use crate::error::AppError;

    with_env(|env| {
        let context = android_context(env)?;
        let coordinator_class = load_app_class(env, &context, ANDROID_SERVICE_COORDINATOR_CLASS)?;
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
            Some(value) => {
                JObject::from(jni_ok(env, "Failed to create stats JSON string", |env| {
                    env.new_string(value)
                })?)
            }
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

/// 处理中（调色/AI 修图）状态同步的全局单调序号（乱序防护，见
/// sync_processing_state）。与 FTP 服务状态同步的 LATEST_SYNC_SEQ
/// 相互独立，避免两条通道互相阻塞或互相跳过。
static LATEST_PROCESSING_SYNC_SEQ: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

/// 串行化处理状态同步 JNI 执行的文件级锁（见 sync_processing_state）。
/// 独立于 SYNC_EXEC_MUTEX。锁中毒仅意味着某次同步任务 panic 过，
/// 互斥语义不受影响，照常使用。
static PROCESSING_SYNC_EXEC_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// 将「调色/AI 修图处理中」组合状态同步到 Kotlin 协调器（前台服务保活）。
///
/// 由 processing_activity 跟踪器在边沿变化时调用；本函数再做一层防御
/// 无妨。内部复刻 sync_android_service_state 的 seq+mutex 乱序防护：
/// spawn_blocking 并发完成的顺序不保证，用独立序号跳过过期任务、用
/// 独立锁串行化 JNI 执行，保证最新状态总是最后生效。
///
/// 使用 Handle::try_current 获取运行时：调用方可能在 worker panic
/// 展开期间的 Drop 守卫里到达这里，直接 spawn_blocking 在无运行时
/// 上下文时会 panic（Drop 内 panic = abort），找不到运行时则记日志跳过。
pub fn sync_processing_state(active: bool, progress_json: String) {
    use std::sync::atomic::Ordering;
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            let seq = LATEST_PROCESSING_SYNC_SEQ.fetch_add(1, Ordering::SeqCst) + 1;
            handle.spawn_blocking(move || {
                // 先取锁串行化：后续任务必须等本次 JNI 完成后才能检查/执行。
                // （poison-tolerant，与 utils/task_worker.rs CancelGate 风格一致）
                let _guard = PROCESSING_SYNC_EXEC_MUTEX
                    .lock()
                    .unwrap_or_else(|p| p.into_inner());
                // 已有更新的状态排队：跳过本次同步，避免乱序覆盖。
                if seq != LATEST_PROCESSING_SYNC_SEQ.load(Ordering::SeqCst) {
                    debug!("skipped stale processing sync seq={}", seq);
                    return;
                }
                if let Err(error) = sync_android_processing_state(active, &progress_json) {
                    error!(%error, active, "Failed to sync Android native processing state");
                }
            });
        }
        Err(_) => {
            // 无 Tokio 运行时上下文（进程关闭期等）：无法发起 JNI，
            // 前台服务生命周期随进程终止，无需补救。
            debug!(
                active,
                "No Tokio runtime context; skipping processing state sync"
            );
        }
    }
}

#[cfg(target_os = "android")]
fn sync_android_processing_state(active: bool, progress_json: &str) -> Result<(), String> {
    use crate::error::AppError;

    with_env(|env| {
        let context = android_context(env)?;
        let coordinator_class = load_app_class(env, &context, ANDROID_SERVICE_COORDINATOR_CLASS)?;
        let progress_arg = JObject::from(jni_ok(
            env,
            "Failed to create processing progress JSON string",
            |env| env.new_string(progress_json),
        )?);

        if let Err(e) = env.call_static_method(
            coordinator_class,
            SYNC_NATIVE_PROCESSING_STATE_METHOD,
            SYNC_NATIVE_PROCESSING_STATE_SIGNATURE,
            &[
                JValue::Object(&context),
                JValue::Bool(active.into()),
                JValue::Object(&progress_arg),
            ],
        ) {
            // 调用失败时可能残留 pending Java 异常，必须清除，
            // 否则当前线程后续所有 JNI 调用都会失败
            clear_pending_exception(env);
            return Err(AppError::Other(format!(
                "Failed to call syncNativeProcessingState: {e}"
            )));
        }

        Ok(())
    })
    .map_err(|e| e.user_message())
}

/// 处理进度同步的全局单调序号（乱序防护，见 sync_processing_progress）。
/// 与启停状态同步的 LATEST_PROCESSING_SYNC_SEQ、FTP 的 LATEST_SYNC_SEQ
/// 相互独立：进度是高频电平数据，独立通道避免互相阻塞或互相跳过。
static LATEST_PROCESSING_PROGRESS_SYNC_SEQ: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

/// 串行化处理进度同步 JNI 执行的文件级锁（见 sync_processing_progress）。
/// 独立于 PROCESSING_SYNC_EXEC_MUTEX / SYNC_EXEC_MUTEX；锁中毒仅意味着
/// 某次同步任务 panic 过，互斥语义不受影响，照常使用。
static PROCESSING_PROGRESS_SYNC_EXEC_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// 将处理进度 JSON（由 processing_activity 组合的 cg/ai 双管线快照）同步
/// 到 Kotlin 协调器（刷新前台服务通知，不触碰服务生命周期）。
///
/// 高频调用（每个 progress 事件一次）：不加 info 日志，仅异常 error、
/// 乱序跳过 debug。内部复刻 sync_processing_state 的 seq+mutex 乱序防护：
/// spawn_blocking 并发完成的顺序不保证，用独立序号跳过过期任务、用独立
/// 锁串行化 JNI 执行，保证最新快照总是最后生效。
pub fn sync_processing_progress(progress_json: String) {
    use std::sync::atomic::Ordering;
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            let seq = LATEST_PROCESSING_PROGRESS_SYNC_SEQ.fetch_add(1, Ordering::SeqCst) + 1;
            handle.spawn_blocking(move || {
                // 先取锁串行化：后续任务必须等本次 JNI 完成后才能检查/执行。
                // （poison-tolerant，与 utils/task_worker.rs CancelGate 风格一致）
                let _guard = PROCESSING_PROGRESS_SYNC_EXEC_MUTEX
                    .lock()
                    .unwrap_or_else(|p| p.into_inner());
                // 已有更新的进度排队：跳过本次同步，避免乱序覆盖。
                if seq != LATEST_PROCESSING_PROGRESS_SYNC_SEQ.load(Ordering::SeqCst) {
                    debug!("skipped stale processing progress sync seq={}", seq);
                    return;
                }
                if let Err(error) = sync_android_processing_progress(&progress_json) {
                    error!(%error, "Failed to sync Android processing progress");
                }
            });
        }
        Err(_) => {
            // 无 Tokio 运行时上下文：无法发起 JNI。进度是电平型数据，
            // 下一次上报自然覆盖，无需补救。
            debug!("No Tokio runtime context; skipping processing progress sync");
        }
    }
}

#[cfg(target_os = "android")]
fn sync_android_processing_progress(progress_json: &str) -> Result<(), String> {
    use crate::error::AppError;

    with_env(|env| {
        let context = android_context(env)?;
        let coordinator_class = load_app_class(env, &context, ANDROID_SERVICE_COORDINATOR_CLASS)?;
        let json_arg = JObject::from(jni_ok(
            env,
            "Failed to create processing progress JSON string",
            |env| env.new_string(progress_json),
        )?);

        if let Err(e) = env.call_static_method(
            coordinator_class,
            SYNC_NATIVE_PROCESSING_PROGRESS_METHOD,
            SYNC_NATIVE_PROCESSING_PROGRESS_SIGNATURE,
            &[JValue::Object(&context), JValue::Object(&json_arg)],
        ) {
            // 调用失败时可能残留 pending Java 异常，必须清除，
            // 否则当前线程后续所有 JNI 调用都会失败
            clear_pending_exception(env);
            return Err(AppError::Other(format!(
                "Failed to call syncNativeProcessingProgress: {e}"
            )));
        }

        Ok(())
    })
    .map_err(|e| e.user_message())
}

// ---------------------------------------------------------------------------
// exit() interposition (Android)
//
// Linker target of `-Wl,--wrap=exit` (see build.rs). tao's Android backend
// ends the process via std::process::exit when its event loop finishes
// (tao-0.35.3 platform_impl/android/mod.rs, EventLoop::run) — i.e. on every
// app close. libc exit() then runs __cxa_finalize, where QNN HTP's
// libQnnHtpPrepare.so GraphPrepare static destructor reliably aborts under
// Scudo ("invalid chunk state", observed on Xiaomi ishtar/SM8550/Android 16
// on every exit after QNN graphs were used); in-process FastRPC teardown on
// this SoC is also known to hang (docs/known-deferred-issues.md §8).
// Terminating via _exit() skips atexit handlers and static destructors
// entirely — leak-on-exit by design; the kernel reclaims memory and FastRPC
// contexts exactly as a task-removal SIGKILL does.
//
// Removal condition: drop this together with the link arg in build.rs once
// the Android exit path no longer runs QNN's destructor (tao/tauri stop
// using libc exit on Android, or QNN fixes their teardown).
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn __wrap_exit(status: i32) -> ! {
    use std::io::Write;
    // Best-effort drain of Rust's line-buffered stdio (the stdout/stderr →
    // logcat pump lives in userspace). Structured logs go through
    // __android_log_write, which is synchronous — nothing else to flush.
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();

    extern "C" {
        fn _exit(status: i32) -> !;
    }
    unsafe { _exit(status) }
}
