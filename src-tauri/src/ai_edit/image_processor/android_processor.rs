// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::path::Path;

use jni::objects::{JClass, JObject, JValue};
use jni::JavaVM;

use super::{ImagePreprocessor, PreparedImage, JPEG_QUALITY, MAX_LONG_SIDE};
use crate::error::AppError;

const BRIDGE_CLASS: &str = "com.gjk.cameraftpcompanion.bridges.ImageProcessorBridge";
const METHOD_NAME: &str = "prepareForUpload";
const METHOD_SIG: &str = "(Ljava/lang/String;II)Ljava/lang/String;";

pub struct AndroidImagePreprocessor;

impl ImagePreprocessor for AndroidImagePreprocessor {
    fn prepare(&self, file_path: &Path) -> Result<PreparedImage, AppError> {
        let path_str = file_path.to_string_lossy().to_string();

        let jvm = get_java_vm()?;
        let mut env = jvm
            .attach_current_thread()
            .map_err(|e| AppError::AiEditError(format!("JNI attach failed: {e}")))?;
        let context = get_android_context(&mut env)?;
        let bridge_class = load_class(&mut env, &context)?;

        let j_path = env
            .new_string(&path_str)
            .map_err(|e| AppError::AiEditError(format!("JNI new_string failed: {e}")))?;

        let result = env
            .call_static_method(
                bridge_class,
                METHOD_NAME,
                METHOD_SIG,
                &[
                    JValue::Object(&JObject::from(j_path)),
                    JValue::Int(MAX_LONG_SIDE as i32),
                    JValue::Int(JPEG_QUALITY as i32),
                ],
            )
            .map_err(|e| AppError::AiEditError(format!("JNI call failed: {e}")))?;

        let j_result = result
            .l()
            .map_err(|e| AppError::AiEditError(format!("JNI result extraction failed: {e}")))?;

        if j_result.is_null() {
            return Err(AppError::AiEditError(
                "Android native image processing failed — likely OOM or unsupported format"
                    .to_string(),
            ));
        }

        let base64: String = env
            .get_string(&j_result.into())
            .map_err(|e| AppError::AiEditError(format!("JNI get_string failed: {e}")))?
            .into();

        Ok(PreparedImage {
            base64_data: base64,
            mime_type: "image/jpeg",
        })
    }
}

fn get_java_vm() -> Result<JavaVM, AppError> {
    let context = ndk_context::android_context();
    unsafe { JavaVM::from_raw(context.vm().cast()) }
        .map_err(|e| AppError::AiEditError(format!("Failed to get JavaVM: {e}")))
}

fn get_android_context<'a>(env: &mut jni::JNIEnv<'a>) -> Result<JObject<'a>, AppError> {
    let context = ndk_context::android_context();
    let raw = unsafe { JObject::from_raw(context.context().cast()) };
    let local = env
        .new_local_ref(&raw)
        .map_err(|e| AppError::AiEditError(format!("Failed to get Android context: {e}")))?;
    let _ = raw.into_raw();
    Ok(local)
}

fn load_class<'a>(
    env: &mut jni::JNIEnv<'a>,
    context: &JObject<'a>,
) -> Result<JClass<'a>, AppError> {
    let loader = env
        .call_method(context, "getClassLoader", "()Ljava/lang/ClassLoader;", &[])
        .and_then(|v| v.l())
        .map_err(|e| AppError::AiEditError(format!("getClassLoader failed: {e}")))?;
    let name = env
        .new_string(BRIDGE_CLASS)
        .map_err(|e| AppError::AiEditError(format!("new_string failed: {e}")))?;
    let class_obj = env
        .call_method(
            loader,
            "loadClass",
            "(Ljava/lang/String;)Ljava/lang/Class;",
            &[JValue::Object(&JObject::from(name))],
        )
        .and_then(|v| v.l())
        .map_err(|e| AppError::AiEditError(format!("loadClass failed: {e}")))?;
    Ok(JClass::from(class_obj))
}
