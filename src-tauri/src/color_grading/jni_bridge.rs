// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! JNI bridge for color grading preview — allows Android's ColorGradingActivity
//! to call Rust preview functions directly without going through Tauri's WebView IPC.

#[cfg(target_os = "android")]
use jni::objects::{JClass, JString};
#[cfg(target_os = "android")]
use jni::sys::{jbyteArray, jfloat, jint, jstring};
#[cfg(target_os = "android")]
use jni::JNIEnv;

#[cfg(target_os = "android")]
use std::sync::OnceLock;

#[cfg(target_os = "android")]
static JNI_RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

/// Run an async future on a JNI thread using a cached multi-threaded Tokio runtime.
/// This is needed because JNI threads are not part of the main Tokio runtime,
/// so `Handle::current()` would panic.
///
/// A multi-threaded runtime is used because multiple JNI threads may call
/// `block_on()` concurrently — `current_thread::Runtime::block_on` is not safe
/// for concurrent use from multiple OS threads.
#[cfg(target_os = "android")]
fn run_blocking<F, T>(fut: F) -> T
where
    F: std::future::Future<Output = T>,
{
    JNI_RUNTIME
        .get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
                .expect("Failed to create JNI tokio runtime")
        })
        .block_on(fut)
}

#[cfg(target_os = "android")]
fn new_json_string(env: &mut JNIEnv, json: &str) -> jstring {
    env.new_string(json)
        .expect("Failed to create JNI string")
        .into_raw()
}

#[cfg(target_os = "android")]
fn json_error(env: &mut JNIEnv, msg: &str) -> jstring {
    let json = serde_json::json!({
        "ok": false,
        "error": msg,
    })
    .to_string();
    new_json_string(env, &json)
}

#[cfg(target_os = "android")]
fn new_json_ok(env: &mut JNIEnv) -> jstring {
    new_json_string(env, r#"{"ok":true}"#)
}

/// JNI: Begin preview session (decode RAW + lens correction).
/// Returns JSON: `{"ok":true}` or `{"ok":false,"error":"message"}`
#[cfg(target_os = "android")]
#[no_mangle]
pub unsafe extern "C" fn Java_com_gjk_cameraftpcompanion_bridges_ColorGradingJniBridge_nativeBeginPreview(
    mut env: JNIEnv,
    _class: JClass,
    file_path: JString,
    half_size: jint,
    max_width: jint,
    max_height: jint,
) -> jstring {
    let path_str = match env.get_string(&file_path) {
        Ok(s) => s.to_string_lossy().into_owned(),
        Err(e) => {
            tracing::error!("JNI beginPreview: failed to read filePath: {e}");
            return json_error(&mut env, "Invalid file path");
        }
    };

    let lensfun_db_path = crate::color_grading::resources::get_resources()
        .ok()
        .map(|r| r.lensfun_db_dir.to_string_lossy().into_owned());

    let state = crate::color_grading::preview::ColorGradingPreviewState::get_global();
    let result = run_blocking(state.begin(
        &path_str,
        lensfun_db_path.as_deref(),
        half_size != 0,
        max_width as u32,
        max_height as u32,
    ));

    match result {
        Ok(()) => new_json_string(&mut env, r#"{"ok":true}"#),
        Err(e) => json_error(&mut env, &e.to_string()),
    }
}

/// JNI: Apply grading to current preview session.
/// Returns JPEG bytes directly as `jbyteArray`, or `null` + throws `RuntimeException` on error.
#[cfg(target_os = "android")]
#[no_mangle]
pub unsafe extern "C" fn Java_com_gjk_cameraftpcompanion_bridges_ColorGradingJniBridge_nativeApplyPreview(
    mut env: JNIEnv,
    _class: JClass,
    lut_id: JString,
    metering_mode: JString,
    ev_offset: jfloat,
    max_width: jint,
    max_height: jint,
) -> jbyteArray {
    let lut_id_str = match env.get_string(&lut_id) {
        Ok(s) => s.to_string_lossy().into_owned(),
        Err(_) => {
            let _ = env.throw_new("java/lang/IllegalArgumentException", "Invalid lutId");
            return std::ptr::null_mut();
        }
    };
    let metering_str = match env.get_string(&metering_mode) {
        Ok(s) => s.to_string_lossy().into_owned(),
        Err(_) => {
            let _ = env.throw_new("java/lang/IllegalArgumentException", "Invalid meteringMode");
            return std::ptr::null_mut();
        }
    };

    let state = crate::color_grading::preview::ColorGradingPreviewState::get_global();
    let result = run_blocking(state.apply(
        &lut_id_str,
        &metering_str,
        ev_offset,
        max_width as u32,
        max_height as u32,
    ));

    match result {
        Ok(jpeg_bytes) => {
            let len = jpeg_bytes.len();
            let arr = match env.new_byte_array(len as i32) {
                Ok(a) => a,
                Err(e) => {
                    tracing::error!("JNI applyPreview: failed to create byte array: {e}");
                    let _ = env.throw_new("java/lang/RuntimeException", "Failed to allocate byte array");
                    return std::ptr::null_mut();
                }
            };
            let signed: &[i8] = unsafe {
                std::slice::from_raw_parts(jpeg_bytes.as_ptr() as *const i8, len)
            };
            if let Err(e) = env.set_byte_array_region(&arr, 0, signed) {
                tracing::error!("JNI applyPreview: failed to set byte array: {e}");
                let _ = env.throw_new("java/lang/RuntimeException", "Failed to copy byte array");
                return std::ptr::null_mut();
            }
            arr.into_raw()
        }
        Err(e) => {
            let msg = e.to_string();
            let _ = env.throw_new("java/lang/RuntimeException", &msg);
            std::ptr::null_mut()
        }
    }
}

/// JNI: End preview session and release resources.
/// Returns JSON: `{"ok":true}` or `{"ok":false,"error":"message"}`
#[cfg(target_os = "android")]
#[no_mangle]
pub unsafe extern "C" fn Java_com_gjk_cameraftpcompanion_bridges_ColorGradingJniBridge_nativeEndPreview(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let state = crate::color_grading::preview::ColorGradingPreviewState::get_global();
    let result = run_blocking(state.end());

    match result {
        Ok(()) => new_json_string(&mut env, r#"{"ok":true}"#),
        Err(e) => json_error(&mut env, &e.to_string()),
    }
}

/// JNI: Get presets list as JSON array.
/// Returns JSON: `[["id1","name1"],["id2","name2"],...]`
#[cfg(target_os = "android")]
#[no_mangle]
pub unsafe extern "C" fn Java_com_gjk_cameraftpcompanion_bridges_ColorGradingJniBridge_nativeGetPresets(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let presets = crate::color_grading::presets::all_presets();
    let json = serde_json::to_string(
        &presets
            .iter()
            .map(|p| vec![p.id.as_str(), p.display_name.as_str()])
            .collect::<Vec<_>>(),
    )
    .unwrap_or_else(|_| "[]".to_string());
    new_json_string(&mut env, &json)
}

/// JNI: Commit full-resolution JPEG from cached RAW and end the session.
/// Returns JSON: `{"ok":true,"outputPath":"..."}` or `{"ok":false,"error":"message"}`
#[cfg(target_os = "android")]
#[no_mangle]
pub unsafe extern "C" fn Java_com_gjk_cameraftpcompanion_bridges_ColorGradingJniBridge_nativeCommitPreview(
    mut env: JNIEnv,
    _class: JClass,
    lut_id: JString,
    metering_mode: JString,
    ev_offset: jfloat,
) -> jstring {
    let lut_id_str = match env.get_string(&lut_id) {
        Ok(s) => s.to_string_lossy().into_owned(),
        Err(_) => return json_error(&mut env, "Invalid lutId"),
    };
    let metering_str = match env.get_string(&metering_mode) {
        Ok(s) => s.to_string_lossy().into_owned(),
        Err(_) => return json_error(&mut env, "Invalid meteringMode"),
    };

    let state = crate::color_grading::preview::ColorGradingPreviewState::get_global();
    let result = run_blocking(state.commit_and_end(
        &lut_id_str,
        &metering_str,
        ev_offset,
    ));

    match result {
        Ok(output_path) => {
            let json = serde_json::json!({"ok": true, "outputPath": output_path}).to_string();
            new_json_string(&mut env, &json)
        }
        Err(e) => json_error(&mut env, &e.to_string()),
    }
}

/// JNI: Get color grading last-used config as JSON.
/// Returns JSON: `{"presetId":"...","evOffset":0.0,"meteringMode":"..."}` or `null`.
#[cfg(target_os = "android")]
#[no_mangle]
pub unsafe extern "C" fn Java_com_gjk_cameraftpcompanion_bridges_ColorGradingJniBridge_nativeGetLastUsed(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let config_service = crate::config_service::ConfigService::get_global();
    match config_service.get() {
        Ok(config) => {
            let json = match &config.color_grading_last_used {
                Some(lu) => serde_json::to_string(lu)
                    .unwrap_or_else(|_| "null".to_string()),
                None => "null".to_string(),
            };
            new_json_string(&mut env, &json)
        }
        Err(_) => new_json_string(&mut env, "null"),
    }
}

/// JNI: Persist color grading last-used config directly to disk.
/// Returns JSON: `{"ok":true}` or `{"ok":false,"error":"message"}`
#[cfg(target_os = "android")]
#[no_mangle]
pub unsafe extern "C" fn Java_com_gjk_cameraftpcompanion_bridges_ColorGradingJniBridge_nativeSaveLastUsed(
    mut env: JNIEnv,
    _class: JClass,
    preset_id: JString,
    metering_mode: JString,
    ev_offset: jfloat,
) -> jstring {
    let preset_id_str = match env.get_string(&preset_id) {
        Ok(s) => s.to_string_lossy().into_owned(),
        Err(_) => return json_error(&mut env, "Invalid presetId"),
    };
    let metering_str = match env.get_string(&metering_mode) {
        Ok(s) => s.to_string_lossy().into_owned(),
        Err(_) => return json_error(&mut env, "Invalid meteringMode"),
    };

    let config_service = crate::config_service::ConfigService::get_global();
    match config_service.mutate_and_persist(|c| {
        c.color_grading_last_used = Some(crate::config::ColorGradingLastUsed {
            preset_id: preset_id_str,
            metering_mode: metering_str,
            ev_offset,
        });
    }) {
        Ok(()) => new_json_ok(&mut env),
        Err(e) => json_error(&mut env, &e.to_string()),
    }
}

/// JNI: Enqueue a single file for batch color grading via the existing worker.
/// Returns JSON: `{"ok":true}` or `{"ok":false,"error":"message"}`
#[cfg(target_os = "android")]
#[no_mangle]
pub unsafe extern "C" fn Java_com_gjk_cameraftpcompanion_bridges_ColorGradingJniBridge_nativeEnqueueBatch(
    mut env: JNIEnv,
    _class: JClass,
    file_path: JString,
    lut_id: JString,
    metering_mode: JString,
    ev_offset: jfloat,
) -> jstring {
    let path_str = match env.get_string(&file_path) {
        Ok(s) => s.to_string_lossy().into_owned(),
        Err(e) => {
            tracing::error!("JNI enqueueBatch: failed to read filePath: {e}");
            return json_error(&mut env, "Invalid file path");
        }
    };
    let lut_id_str = match env.get_string(&lut_id) {
        Ok(s) => s.to_string_lossy().into_owned(),
        Err(_) => return json_error(&mut env, "Invalid lutId"),
    };
    let metering_str = match env.get_string(&metering_mode) {
        Ok(s) => s.to_string_lossy().into_owned(),
        Err(_) => return json_error(&mut env, "Invalid meteringMode"),
    };

    let service = crate::color_grading::service::ColorGradingService::get_global();
    let result = run_blocking(service.enqueue(
        vec![std::path::PathBuf::from(path_str)],
        lut_id_str,
        metering_str,
        ev_offset,
    ));

    match result {
        Ok(()) => new_json_string(&mut env, r#"{"ok":true}"#),
        Err(e) => json_error(&mut env, &e.to_string()),
    }
}
