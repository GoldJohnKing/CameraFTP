// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! 轻量处理活跃跟踪器：聚合调色（color grading）与 AI 修图（AI edit）
//! 两条后台管线的忙闲状态，仅在「组合忙闲」发生边沿变化时向 Android
//! 前台服务通道同步一次。上传百张照片触发百次 enqueue 上报时，JNI
//! 只发生一次（false→true 边沿）。
//!
//! 语义约定：
//! - 某管线「忙」= 队列非空 **或** 有任务正在处理中（单 worker 取走任务后
//!   queue_depth 即减到 0，in-flight 必须计入，否则最后一个任务处理到一半
//!   前台服务就会被撤销）。
//! - 两条管线任一忙 → 组合忙；全部空闲 → 组合闲（combined 边沿，保证
//!   两条管线同时忙、一条先结束时前台服务仍保持）。
//!
//! 纯观测：本模块不干预队列处理本身（不暂停、不限速）。非 Android 平台
//! 上报仅更新标志，不发起 JNI（[`dispatch_platform_sync`] no-op）。

use std::sync::atomic::{AtomicBool, Ordering};
use tracing::info;

/// 调色管线忙闲标志。
static CG_ACTIVE: AtomicBool = AtomicBool::new(false);

/// AI 修图管线忙闲标志。
static AI_ACTIVE: AtomicBool = AtomicBool::new(false);

/// 上次已向平台同步的组合活跃值（边沿检测基准）。
static SYNCED_COMBINED_ACTIVE: AtomicBool = AtomicBool::new(false);

/// 上报调色管线忙闲（忙 = 队列非空或有任务处理中）。
pub fn notify_cg(active: bool) {
    notify_one("color-grading", active, &CG_ACTIVE);
}

/// 上报 AI 修图管线忙闲（忙 = 队列非空或有任务处理中）。
pub fn notify_ai(active: bool) {
    notify_one("ai-edit", active, &AI_ACTIVE);
}

/// 更新单管线标志并做组合边沿检测。
///
/// 边沿检测用 `swap` 原子完成：并发上报最终收敛——最后一次 swap 看到的
/// 组合值与真实组合值一致时才触发同步。已知瞬态：若 enqueue 恰落在
/// worker 循环顶部的 `depth.get()` 与本函数的 flag store 之间，worker
/// 会按旧深度（0）上报一次 false，紧随其后的 enqueue true 再翻转，产生
/// 一次瞬态 true→false→true 的前台服务停/启抖动（仅多一次往返，无功能
/// 影响）。SeqCst 全序 + 每条管线「激活上报先于其任务完成上报」的
/// happens-before 链（enqueue/send/receiver）保证最终一致：后续任一次
/// 上报都会把 synced 值拉回真实组合值，不会卡死在 false。
fn notify_one(service: &str, active: bool, flag: &AtomicBool) {
    flag.store(active, Ordering::SeqCst);
    let combined = CG_ACTIVE.load(Ordering::SeqCst) || AI_ACTIVE.load(Ordering::SeqCst);
    // swap 返回旧值：仅边沿变化时同步（连续 100 次 enqueue(true) 只同步 1 次）。
    let previous = SYNCED_COMBINED_ACTIVE.swap(combined, Ordering::SeqCst);
    if previous != combined {
        info!(
            service = service,
            from = previous,
            to = combined,
            cg_active = CG_ACTIVE.load(Ordering::SeqCst),
            ai_active = AI_ACTIVE.load(Ordering::SeqCst),
            "Processing activity edge changed"
        );
        dispatch_platform_sync(combined);
    }
}

#[cfg(target_os = "android")]
fn dispatch_platform_sync(combined: bool) {
    crate::platform::android::sync_processing_state(combined);
}

#[cfg(not(target_os = "android"))]
fn dispatch_platform_sync(_combined: bool) {
    // 非 Android：无前台服务概念，纯 no-op（标志已在 notify_one 更新）。
}
