// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::Path;

use jni::objects::{JObject, JValue};

use super::{ImagePreprocessor, PreparedImage, JPEG_QUALITY, MAX_LONG_SIDE};
use crate::error::AppError;
use crate::utils::jni::{
    android_context, clear_pending_exception, jni_ok, load_app_class, with_env,
};

const BRIDGE_CLASS: &str = "com.gjk.cameraftpcompanion.bridges.ImageProcessorBridge";
const METHOD_NAME: &str = "prepareForUpload";
const METHOD_SIG: &str = "(Ljava/lang/String;II)Ljava/lang/String;";

pub struct AndroidImagePreprocessor;

impl ImagePreprocessor for AndroidImagePreprocessor {
    fn prepare(&self, file_path: &Path) -> Result<PreparedImage, AppError> {
        let path_str = file_path.to_string_lossy().to_string();

        with_env(|env| {
            let context = android_context(env)?;
            let bridge_class = load_app_class(env, &context, BRIDGE_CLASS)?;

            let j_path =
                jni_ok(env, "JNI new_string failed", |env| env.new_string(&path_str))?;

            let result = match env.call_static_method(
                bridge_class,
                METHOD_NAME,
                METHOD_SIG,
                &[
                    JValue::Object(&JObject::from(j_path)),
                    JValue::Int(MAX_LONG_SIDE as i32),
                    JValue::Int(JPEG_QUALITY as i32),
                ],
            ) {
                Ok(result) => result,
                Err(e) => {
                    // 调用失败时可能残留 pending Java 异常，必须清除，
                    // 否则当前线程后续所有 JNI 调用都会失败
                    clear_pending_exception(env);
                    return Err(AppError::AiEditError(format!("JNI call failed: {e}")));
                }
            };

            let j_result = result
                .l()
                .map_err(|e| AppError::AiEditError(format!("JNI result extraction failed: {e}")))?;

            if j_result.is_null() {
                return Err(AppError::AiEditError(
                    "Android native image processing failed — likely OOM or unsupported format"
                        .to_string(),
                ));
            }

            let base64: String = match env.get_string(&j_result.into()) {
                Ok(s) => s.into(),
                Err(e) => {
                    // 读取结果字符串可能残留 pending Java 异常（如 OOM），
                    // 必须清除，否则当前线程后续 JNI 调用都会失败
                    clear_pending_exception(env);
                    return Err(AppError::AiEditError(format!("JNI get_string failed: {e}")));
                }
            };

            Ok(PreparedImage {
                base64_data: base64,
                mime_type: "image/jpeg",
            })
        })
        .map_err(|e| match e {
            // env 操作失败的 AiEditError 原样透传；JNI 引导阶段的
            // AppError（JavaVM/attach/Context/loadClass）按原实现语义
            // 统一包装为 AiEditError。
            AppError::AiEditError(_) => e,
            other => AppError::AiEditError(other.user_message()),
        })
    }
}
