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
//!
//! ndk-context 由应用自初始化（见下方 [`init_ndk_context`] 一节）：
//! tauri 2.11.x（tauri-runtime-wry 2.11.4 → tao 0.35.3）不再代为初始化。

use std::ffi::c_void;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Once, OnceLock};

use jni::objects::{GlobalRef, JClass, JObject, JValue};
use jni::{JNIEnv, JavaVM};

use crate::error::AppError;

// ---------------------------------------------------------------------------
// ndk-context 自初始化（tauri 2.11.x / tao 0.35.3 回归的临时 workaround）
// ---------------------------------------------------------------------------
//
// 上游链：tauri 2.11.5 → tauri-runtime-wry 2.11.4 → tao 0.35.3 不再初始化
// ndk-context（上游回归，tao#1220/#1266；tao 0.36 修复，tauri 2.12 未发布）。
// 未初始化时 `ndk_context::android_context()` 直接 panic，本文件所有 JNI
// 桥（FTP MediaStore、服务状态同步、AI 修图预处理）全部失效。
//
// 修复：MainActivity.onCreate 在 super.onCreate()（内部经
// WryActivity.onCreate → Rust.onActivityCreate 完成
// System.loadLibrary("camera_ftp_companion_lib")）之后立即回调
// [`Java_com_gjk_cameraftpcompanion_MainActivity_initNdkContext`]，应用侧
// 自行初始化 ndk-context。
//
// **移除条件**：tauri ≥ 2.12 发布后应整体移除本节，连同 Kotlin 侧
// MainActivity 的 external fun 声明与 onCreate 调用。

/// Context 全局引用的进程级持有槽。
///
/// ndk-context 拿走裸指针后进程期内一直使用，因此该 GlobalRef **永不删除**；
/// 存入静态槽防 Drop 与编译器优化。
static NDK_CONTEXT_KEEPALIVE: OnceLock<GlobalRef> = OnceLock::new();

/// 保证初始化逻辑只执行一次（Activity 重建会重复调用 JNI 入口）。
static NDK_CONTEXT_INIT_ONCE: Once = Once::new();

/// 初始化结果（用于区分日志级别；不外泄）。
enum InitOutcome {
    Initialized,
    /// ndk-context 已被别处初始化（未来 tao ≥ 0.36 恢复自身初始化时的
    /// 先到者赢场景）。
    AlreadyInitializedElsewhere(String),
    Failed(String),
}

/// MainActivity.initNdkContext 的 JNI 入口；符号名与 Kotlin 侧
/// `private external fun initNdkContext(context: Context)`（实例方法）逐字对应。
///
/// 双初始化防护：ndk-context 0.1.1 的 `initialize_android_context` 在已
/// 初始化时 assert panic，这里用 catch_unwind 捕获后仅记 debug 日志——
/// 先到者赢、后到者无害。（catch 在 `call_once` 闭包**内部**，Once 不会被
/// 毒化，后续重入不会二次 panic。）
///
/// extern "C" 边界绝不外泄 panic：最外层再包一层 catch_unwind，一切失败
/// 只打日志。
#[no_mangle]
pub extern "C" fn Java_com_gjk_cameraftpcompanion_MainActivity_initNdkContext(
    env: JNIEnv<'_>,
    _this: JObject<'_>,
    context: JObject<'_>,
) {
    let outcome = catch_unwind(AssertUnwindSafe(|| init_ndk_context(env, context)));
    match outcome {
        Ok(InitOutcome::Initialized) => {
            tracing::info!("ndk-context initialized from MainActivity");
        }
        Ok(InitOutcome::AlreadyInitializedElsewhere(msg)) => {
            tracing::debug!("ndk-context already initialized elsewhere, skipping: {msg}");
        }
        Ok(InitOutcome::Failed(reason)) => {
            tracing::error!("ndk-context self-init failed: {reason}");
        }
        Err(payload) => {
            tracing::error!(
                "ndk-context self-init panicked (unexpected): {}",
                panic_message(payload)
            );
        }
    }
}

/// 幂等执行一次性初始化并返回结果。
fn init_ndk_context(env: JNIEnv<'_>, context: JObject<'_>) -> InitOutcome {
    let mut outcome = InitOutcome::Failed("init closure did not run".to_string());
    NDK_CONTEXT_INIT_ONCE.call_once(|| {
        outcome = run_ndk_context_init(env, context);
    });
    outcome
}

/// 实际初始化：JavaVM 裸指针 + Context 全局引用 → ndk-context。
fn run_ndk_context_init(env: JNIEnv<'_>, context: JObject<'_>) -> InitOutcome {
    let mut env = env;
    // get_java_vm / new_global_ref 只会返回 JNI 错误（不抛 Java 异常），
    // 但仍走 jni_ok 统一清理 pending 异常，与本模块约定一致。
    let vm = match jni_ok(&mut env, "Failed to get JavaVM for ndk-context init", |e| {
        e.get_java_vm()
    }) {
        Ok(vm) => vm,
        Err(e) => return InitOutcome::Failed(format!("{e}")),
    };
    let global = match jni_ok(
        &mut env,
        "Failed to create global context ref for ndk-context",
        |e| e.new_global_ref(&context),
    ) {
        Ok(global) => global,
        Err(e) => return InitOutcome::Failed(format!("{e}")),
    };

    // 指针类型严格对照 ndk-context 0.1.1 签名：
    // initialize_android_context(java_vm: *mut c_void, context_jobject: *mut c_void)
    let vm_ptr = vm.get_java_vm_pointer().cast::<c_void>();
    let ctx_ptr = global.as_obj().as_raw().cast::<c_void>();

    // 注意：ndk-context 0.1.1 的 initialize 内部是"先 replace 后 assert"——
    // 即便随后的 assert panic（双初始化），存入静态的也是**我们的**新指针。
    // 因此 keepalive 必须在成功与双初始化两条路径都保留，不能只留成功路径。
    let _ = NDK_CONTEXT_KEEPALIVE.set(global);

    // SAFETY: vm/ctx 指针来自 get_java_vm / new_global_ref，进程期内有效；
    // global 已存入静态槽永不删除。已初始化时内部 assert panic，由
    // catch_unwind 捕获（闭包内仅捕获裸指针，UnwindSafe）。
    match catch_unwind(|| unsafe { ndk_context::initialize_android_context(vm_ptr, ctx_ptr) }) {
        Ok(()) => InitOutcome::Initialized,
        Err(payload) => InitOutcome::AlreadyInitializedElsewhere(panic_message(payload)),
    }
}

/// 把 panic payload 转成可读字符串（仅用于日志）。
fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "<non-string panic payload>".to_string())
}

/// 读取 ndk-context；未初始化时 ndk-context 0.1.1 会直接 panic
/// （`expect("android context was not initialized")`），无法从返回值拦截。
/// 这里用 catch_unwind 把 panic 转成带修复指引的 `AppError`。
///
/// 注：未初始化时 default panic hook 会先往 logcat 打一条 panic 记录
/// （仅在自初始化失败的破损状态下发生），属预期诊断噪音。
fn ndk_context_or_hint() -> Result<ndk_context::AndroidContext, AppError> {
    catch_unwind(|| ndk_context::android_context()).map_err(|_| {
        AppError::Other(
            "ndk-context not initialized; MainActivity.initNdkContext must run first \
             (tauri 2.11/tao 0.35 regression)"
                .to_string(),
        )
    })
}

/// 获取进程级 JavaVM（由 Android 运行时经 ndk-context 提供）。
fn java_vm() -> Result<JavaVM, AppError> {
    let context = ndk_context_or_hint()?;
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
    let context = ndk_context_or_hint()?;
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
    let class_obj = jni_ok(env, &format!("Failed to load class {class_name}"), |env| {
        env.call_method(
            loader,
            "loadClass",
            "(Ljava/lang/String;)Ljava/lang/Class;",
            &[JValue::Object(&class_name_obj)],
        )
        .and_then(|v| v.l())
    })?;

    Ok(JClass::from(class_obj))
}
