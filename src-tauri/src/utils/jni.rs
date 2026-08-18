// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Android JNI 引导助手：统一 "JVM attach + Context local ref +
//! getClassLoader + loadClass" 协议，供平台服务状态同步、AI 修图预处理
//! 与 MediaStore 桥接共用。
//!
//! 约定：所有会抛出 Java 异常的 JNI 调用一律经 [`jni_ok`] 守卫（或在
//! Err 分支等效地 describe + clear）后再返回——带着未清异常继续 JNI
//! 调用会让后续调用立即失败，甚至触发 JVM abort。

use jni::objects::{JClass, JObject, JValue};
use jni::{JNIEnv, JavaVM};

use crate::error::AppError;

/// 获取进程级 JavaVM（由 Android 运行时经 ndk-context 提供）。
fn java_vm() -> Result<JavaVM, AppError> {
    let context = ndk_context::android_context();
    // SAFETY: vm pointer is provided by the Android runtime via ndk-context
    // and stays valid for the process lifetime.
    unsafe { JavaVM::from_raw(context.vm().cast()) }
        .map_err(|e| AppError::Other(format!("Failed to get JavaVM: {e}")))
}

/// 将当前线程 attach 到 JVM，并在闭包中执行 JNI 调用。
///
/// 调用方负责把 `AppError` 映射回自己的错误类型；各失败分支的
/// `AppError::Other` 文案见本模块内 format!。
pub fn with_env<T>(f: impl FnOnce(&mut JNIEnv<'_>) -> Result<T, AppError>) -> Result<T, AppError> {
    let vm = java_vm()?;
    let mut env = vm
        .attach_current_thread()
        .map_err(|e| AppError::Other(format!("Failed to attach JNI thread: {e}")))?;
    f(&mut env)
}

/// 检查并清除 pending Java 异常。
///
/// JNI 调用失败后线程上可能残留未处理的 Java 异常，不清除会导致
/// 后续所有 JNI 调用立即失败。
pub fn clear_pending_exception(env: &mut JNIEnv<'_>) {
    match env.exception_check() {
        Ok(true) => {
            let _ = env.exception_describe();
            let _ = env.exception_clear();
        }
        _ => {}
    }
}

/// 通用 JNI 守卫：执行可能抛出 Java 异常的 JNI 调用并统一清理。
///
/// `op` 返回 Err 时，若线程上残留 pending Java 异常则 describe + clear
/// （复用 [`clear_pending_exception`]），再返回带 `what` 上下文的
/// `AppError::Other("{what}: {err}")`；无 pending 异常的失败（如参数
/// 类型错误）时 clear 为 no-op，同样映射为带上下文的错误。
pub(crate) fn jni_ok<'env, T>(
    env: &mut JNIEnv<'env>,
    what: &str,
    op: impl FnOnce(&mut JNIEnv<'env>) -> Result<T, jni::errors::Error>,
) -> Result<T, AppError> {
    match op(env) {
        Ok(value) => Ok(value),
        Err(err) => {
            clear_pending_exception(env);
            Err(AppError::Other(format!("{what}: {err}")))
        }
    }
}

/// 获取全局 Android Application Context 的 local ref。
pub fn android_context<'a>(env: &mut JNIEnv<'a>) -> Result<JObject<'a>, AppError> {
    let context = ndk_context::android_context();
    // SAFETY: context pointer is managed by the Android runtime and stays
    // valid for the process lifetime.
    let raw = unsafe { JObject::from_raw(context.context().cast()) };
    let local = jni_ok(env, "Failed to create local Android context ref", |env| {
        env.new_local_ref(&raw)
    })?;
    // 全局 context 指针由 Android 运行时管理：放弃包装的所有权，避免 Drop 释放它。
    let _ = raw.into_raw();
    Ok(local)
}

/// 通过 Application Context 的 ClassLoader 加载应用类。
pub fn load_app_class<'a>(
    env: &mut JNIEnv<'a>,
    ctx: &JObject<'a>,
    class_name: &str,
) -> Result<JClass<'a>, AppError> {
    let loader = jni_ok(env, "Failed to get app ClassLoader", |env| {
        env.call_method(ctx, "getClassLoader", "()Ljava/lang/ClassLoader;", &[])
            .and_then(|v| v.l())
    })?;
    let class_name_str = jni_ok(env, "Failed to create class-name string", |env| {
        env.new_string(class_name)
    })?;
    let class_name_obj = JObject::from(class_name_str);
    let class_obj = jni_ok(
        env,
        &format!("Failed to load class {class_name}"),
        |env| {
            env.call_method(
                loader,
                "loadClass",
                "(Ljava/lang/String;)Ljava/lang/Class;",
                &[JValue::Object(&class_name_obj)],
            )
            .and_then(|v| v.l())
        },
    )?;

    Ok(JClass::from(class_obj))
}
