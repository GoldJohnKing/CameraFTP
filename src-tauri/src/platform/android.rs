// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::traits::PlatformService;
use super::types::{PermissionStatus, StorageInfo};
use crate::constants::ANDROID_DCIM_PATH;
use crate::ftp::types::ServerStateSnapshot;
use crate::utils::fs::is_path_writable;
use tauri::{AppHandle, Emitter};
use tracing::{debug, error, info};

#[cfg(target_os = "android")]
use jni::objects::{JClass, JObject, JValue};
#[cfg(target_os = "android")]
use jni::JavaVM;

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

/// 获取存储路径信息
pub fn get_storage_info() -> StorageInfo {
    let path = DEFAULT_STORAGE_PATH;
    let path_buf = std::path::PathBuf::from(path);

    let exists = path_buf.exists();
    let writable = if exists {
        validate_path_writable(path)
    } else {
        false
    };

    // 检查权限：如果能写入，就认为有权限
    let has_all_files_access = writable || (exists && can_write_to_dcim());

    StorageInfo {
        display_name: STORAGE_DISPLAY_NAME.to_string(),
        path: path.to_string(),
        exists,
        writable,
        has_all_files_access,
    }
}

/// 检查权限状态
pub fn check_permission_status() -> PermissionStatus {
    let has_access = check_media_store_permission();
    PermissionStatus {
        has_all_files_access: has_access,
        needs_user_action: !has_access,
    }
}

/// 检查是否有媒体存储权限
/// 权限检查现在通过 Kotlin bridge 完成；假设如果可以查询 MediaStore 就已授权
fn check_media_store_permission() -> bool {
    // Permission check now done via Kotlin bridge; assume granted if we can query MediaStore
    true
}

/// 检查 DCIM 目录是否可写（用于判断所有文件访问权限）
fn can_write_to_dcim() -> bool {
    let dcim_path = std::path::Path::new(ANDROID_DCIM_PATH);
    if !dcim_path.exists() {
        debug!("DCIM path does not exist");
        return false;
    }
    let writable = is_path_writable(dcim_path);
    if writable {
        debug!("All files access permission: granted (DCIM writable)");
    } else {
        debug!("All files access permission: denied (DCIM not writable)");
    }
    writable
}

/// 验证路径是否可写
fn validate_path_writable(path: &str) -> bool {
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
    let writable = is_path_writable(&path_buf);
    if writable {
        debug!("Path is writable: {:?}", path_buf);
    } else {
        error!("Path is not writable: {:?}", path_buf);
    }
    writable
}

/// 确保存储目录存在且可写
/// 前端通过 PermissionDialog 处理权限检查，这里只负责创建目录
pub fn ensure_storage_ready() -> Result<String, String> {
    let path = DEFAULT_STORAGE_PATH;
    let path_buf = std::path::PathBuf::from(path);

    // 创建目录（如果不存在）
    // 前端已处理权限检查，这里直接尝试创建目录
    if !path_buf.exists() {
        std::fs::create_dir_all(&path_buf).map_err(|e| format!("无法创建存储目录: {}", e))?;
        info!("Created storage directory: {}", path);
    }

    Ok(path.to_string())
}

/// 打开存储权限设置页面
pub fn open_storage_permission_settings(app: &AppHandle) {
    let _ = app.emit("android-open-storage-permission-settings", ());
    info!("Requesting READ_MEDIA_IMAGES permission");
}

/// Android 平台实现
pub struct AndroidPlatform;

impl PlatformService for AndroidPlatform {
    fn name(&self) -> &'static str {
        "android"
    }

    fn setup(&self, _app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
        tracing::info!("Android platform initialized");
        Ok(())
    }

    fn get_storage_info(&self) -> StorageInfo {
        get_storage_info()
    }

    fn check_permission_status(&self) -> PermissionStatus {
        check_permission_status()
    }

    fn ensure_storage_ready(&self, _app: &AppHandle) -> Result<String, String> {
        ensure_storage_ready()
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
            if let Err(error) = sync_android_service_state(snapshot) {
                error!(%error, ?snapshot, "Failed to sync Android native service state");
                return;
            }
        }

        info!(
            ?snapshot,
            "Syncing Android native service state from Rust events"
        );
    }

    fn get_default_storage_path(&self) -> std::path::PathBuf {
        std::path::PathBuf::from(DEFAULT_STORAGE_PATH)
    }

    fn request_all_files_permission(&self, app: &AppHandle) -> Result<bool, String> {
        let status = self.check_permission_status();
        if status.needs_user_action {
            open_storage_permission_settings(app);
            info!("Requested READ_MEDIA_IMAGES permission");
            Ok(false) // User must grant via system dialog
        } else {
            Ok(true)
        }
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
    let jvm = get_java_vm()?;
    let mut env = jvm
        .attach_current_thread()
        .map_err(|e| format!("Failed to attach JNI thread: {e}"))?;
    let context = get_android_context(&mut env)?;
    let coordinator_class = get_coordinator_class(&mut env, &context)?;
    let stats_json = match serde_json::to_string(snapshot) {
        Ok(value) if snapshot.is_running => Some(value),
        Ok(_) => None,
        Err(e) => return Err(format!("Failed to serialize service snapshot: {e}")),
    };
    let stats_arg = match stats_json.as_deref() {
        Some(value) => JObject::from(
            env.new_string(value)
                .map_err(|e| format!("Failed to create stats JSON string: {e}"))?,
        ),
        None => JObject::null(),
    };
    let connected_clients = i32::try_from(snapshot.connected_clients).map_err(|_| {
        format!(
            "Connected client count exceeds Android JNI range: {}",
            snapshot.connected_clients
        )
    })?;

    env.call_static_method(
        coordinator_class,
        SYNC_ANDROID_SERVICE_STATE_METHOD,
        SYNC_ANDROID_SERVICE_STATE_SIGNATURE,
        &[
            JValue::Object(&context),
            JValue::Bool(snapshot.is_running.into()),
            JValue::Object(&stats_arg),
            JValue::Int(connected_clients),
        ],
    )
    .map_err(|e| format!("Failed to call syncNativeServiceState: {e}"))?;

    Ok(())
}

#[cfg(target_os = "android")]
fn get_java_vm() -> Result<JavaVM, String> {
    let context = ndk_context::android_context();
    unsafe { JavaVM::from_raw(context.vm().cast()) }
        .map_err(|e| format!("Failed to get JavaVM: {e}"))
}

#[cfg(target_os = "android")]
fn get_android_context<'a>(env: &mut jni::JNIEnv<'a>) -> Result<JObject<'a>, String> {
    let context = ndk_context::android_context();
    let raw_context = unsafe { JObject::from_raw(context.context().cast()) };
    let local_context = env
        .new_local_ref(&raw_context)
        .map_err(|e| format!("Failed to create local Android context ref: {e}"))?;
    let _ = raw_context.into_raw();
    Ok(local_context)
}

#[cfg(target_os = "android")]
fn get_coordinator_class<'a>(
    env: &mut jni::JNIEnv<'a>,
    context: &JObject<'a>,
) -> Result<JClass<'a>, String> {
    let loader = env
        .call_method(context, "getClassLoader", "()Ljava/lang/ClassLoader;", &[])
        .and_then(|value| value.l())
        .map_err(|e| format!("Failed to get Android app ClassLoader: {e}"))?;
    let class_name = env
        .new_string(ANDROID_SERVICE_COORDINATOR_CLASS)
        .map_err(|e| format!("Failed to create AndroidServiceStateCoordinator class name: {e}"))?;
    let class_name_obj = JObject::from(class_name);
    let class_obj = env
        .call_method(
            loader,
            "loadClass",
            "(Ljava/lang/String;)Ljava/lang/Class;",
            &[JValue::Object(&class_name_obj)],
        )
        .and_then(|value| value.l())
        .map_err(|e| {
            format!("Failed to load AndroidServiceStateCoordinator with app ClassLoader: {e}")
        })?;

    Ok(JClass::from(class_obj))
}

#[cfg(test)]
mod tests {
    #[test]
    fn android_platform_exposes_native_service_sync_hook() {
        let source = include_str!("traits.rs");

        assert!(source.contains("sync_android_service_state"));
    }

    #[test]
    fn android_platform_no_longer_emits_service_state_update_event() {
        let source = include_str!("android.rs");
        let stale_event = ["android-service", "-state-update"].concat();

        assert!(!source.contains(&stale_event));
    }

    #[test]
    fn android_platform_uses_context_class_loader_for_coordinator_lookup() {
        let source = include_str!("android.rs");

        assert!(source.contains("getClassLoader"));
        assert!(source.contains("loadClass"));
        assert!(!source.contains("find_class(ANDROID_SERVICE_COORDINATOR_CLASS)"));
    }

    #[test]
    fn android_service_coordinator_is_kept_for_release_jni_calls() {
        let rules = include_str!("../../gen/android/app/proguard-rules.pro");

        assert!(rules.contains("AndroidServiceStateCoordinator"));
    }
}
