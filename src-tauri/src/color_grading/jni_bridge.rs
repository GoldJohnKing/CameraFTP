// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! JNI bridge for color grading preview — allows Android's ColorGradingActivity
//! to call Rust preview functions directly without going through Tauri's WebView IPC.

#[cfg(target_os = "android")]
use jni::objects::{JClass, JString};
#[cfg(target_os = "android")]
use jni::sys::{jfloat, jstring};
#[cfg(target_os = "android")]
use jni::JNIEnv;

/// Run an async future on a JNI thread by creating a temporary single-threaded
/// Tokio runtime. This is needed because JNI threads are not part of the main
/// Tokio runtime, so `Handle::current()` would panic.
///
/// `tokio::sync::Mutex` works correctly across runtimes: the lock acquisition
/// uses atomic operations and the waker from whichever runtime polls the future.
#[cfg(target_os = "android")]
fn run_blocking<F, T>(fut: F) -> T
where
    F: std::future::Future<Output = T>,
{
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to create JNI tokio runtime")
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
    new_json_string(
        env,
        &format!(
            r#"{{"ok":false,"error":"{}"}}"#,
            msg.replace('\\', "\\\\").replace('"', "\\\"")
        ),
    )
}

/// JNI: Begin preview session (decode RAW + lens correction).
/// Returns JSON: `{"ok":true}` or `{"ok":false,"error":"message"}`
#[cfg(target_os = "android")]
#[no_mangle]
pub unsafe extern "C" fn Java_com_gjk_cameraftpcompanion_bridges_ColorGradingJniBridge_nativeBeginPreview(
    mut env: JNIEnv,
    _class: JClass,
    file_path: JString,
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
    let result = run_blocking(state.begin(&path_str, lensfun_db_path.as_deref()));

    match result {
        Ok(()) => new_json_string(&mut env, r#"{"ok":true}"#),
        Err(e) => json_error(&mut env, &e.to_string()),
    }
}

/// JNI: Apply grading to current preview session.
/// Returns JSON: `{"ok":true,"url":"..."}` or `{"ok":false,"error":"message"}`
#[cfg(target_os = "android")]
#[no_mangle]
pub unsafe extern "C" fn Java_com_gjk_cameraftpcompanion_bridges_ColorGradingJniBridge_nativeApplyPreview(
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
    let result = run_blocking(state.apply(&lut_id_str, true, &metering_str, ev_offset));

    match result {
        Ok(url) => new_json_string(
            &mut env,
            &format!(
                r#"{{"ok":true,"url":"{}"}}"#,
                url.replace('\\', "\\\\").replace('"', "\\\"")
            ),
        ),
        Err(e) => json_error(&mut env, &e.to_string()),
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
