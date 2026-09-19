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
//! 除忙闲边沿外，本模块还承载各管线的**实时进度**（[`PipelineProgress`]）：
//! worker 在每个 progress 事件 emit 点位旁上报同数值快照，服务活跃期间
//! 组合两槽位为 JSON 推给 Kotlin 刷新通知。进度是电平型数据（最新者胜），
//! 无自己的边沿检测；管线失活上报（notify_cg/notify_ai 传 false）清空
//! 对应进度槽。
//!
//! 纯观测：本模块不干预队列处理本身（不暂停、不限速）。非 Android 平台
//! 上报仅更新标志，不发起 JNI（[`dispatch_platform_sync`] 与
//! [`dispatch_progress_sync`] 均 no-op）。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tracing::info;

/// 调色管线忙闲标志。
static CG_ACTIVE: AtomicBool = AtomicBool::new(false);

/// AI 修图管线忙闲标志。
static AI_ACTIVE: AtomicBool = AtomicBool::new(false);

/// 上次已向平台同步的组合活跃值（边沿检测基准）。
static SYNCED_COMBINED_ACTIVE: AtomicBool = AtomicBool::new(false);

/// 单管线进度快照（done/total/failed），数值与对应前端 progress 事件同源。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PipelineProgress {
    pub done: u32,
    pub total: u32,
    pub failed: u32,
}

/// 调色管线最新进度槽位（None = 无任务；失活上报时清空）。
static CG_PROGRESS: Mutex<Option<PipelineProgress>> = Mutex::new(None);

/// AI 修图管线最新进度槽位（None = 无任务；失活上报时清空）。
static AI_PROGRESS: Mutex<Option<PipelineProgress>> = Mutex::new(None);

/// 上报调色管线忙闲（忙 = 队列非空或有任务处理中）。
pub fn notify_cg(active: bool) {
    notify_one("color-grading", active, &CG_ACTIVE, &CG_PROGRESS);
}

/// 上报 AI 修图管线忙闲（忙 = 队列非空或有任务处理中）。
pub fn notify_ai(active: bool) {
    notify_one("ai-edit", active, &AI_ACTIVE, &AI_PROGRESS);
}

/// 上报调色管线进度（worker 在每个 `color-grading-progress` 事件点位旁调用，
/// 数值与事件完全相同）。仅服务已活跃时向平台分发。
pub fn notify_cg_progress(progress: PipelineProgress) {
    notify_progress(&CG_PROGRESS, progress);
}

/// 上报 AI 修图管线进度（worker 在每个 `ai-edit-progress` 事件点位旁调用，
/// 数值与事件完全相同）。仅服务已活跃时向平台分发。
pub fn notify_ai_progress(progress: PipelineProgress) {
    notify_progress(&AI_PROGRESS, progress);
}

/// 入队合并推送（调色）：保留槽内 done/failed（运行中批次的已完成/失败
/// 计数），total = done + depth。空槽（空闲起步）时即 {0, depth, 0}。
/// 用于 enqueue 路径——运行中途追加任务时通知的 done 不得回零、total
/// 随之增长（修复：此前裸覆盖 {0, N}，追加图片后已完成数被清零）。
pub fn notify_cg_enqueued(depth: u32) {
    notify_enqueued(&CG_PROGRESS, depth);
}

/// 入队合并推送（AI 修图），语义同 [`notify_cg_enqueued`]。
pub fn notify_ai_enqueued(depth: u32) {
    notify_enqueued(&AI_PROGRESS, depth);
}

fn notify_enqueued(slot: &Mutex<Option<PipelineProgress>>, depth: u32) {
    let cur = read_progress(slot).unwrap_or(PipelineProgress {
        done: 0,
        total: 0,
        failed: 0,
    });
    notify_progress(
        slot,
        PipelineProgress {
            done: cur.done,
            total: cur.done + depth,
            failed: cur.failed,
        },
    );
}

/// 存入进度槽位；仅在组合活跃已同步为 true 时分发（服务未启动不刷——
/// 启停生命周期完全由边沿通道管，进度通道绝不拉起服务）。
fn notify_progress(slot: &Mutex<Option<PipelineProgress>>, progress: PipelineProgress) {
    write_progress(slot, Some(progress));
    if SYNCED_COMBINED_ACTIVE.load(Ordering::SeqCst) {
        dispatch_progress_sync();
    }
}

/// 读写进度槽位。锁中毒仅意味着某次持锁线程 panic 过，互斥语义不受
/// 影响，照常使用（与 android.rs PROCESSING_SYNC_EXEC_MUTEX 风格一致）。
fn write_progress(slot: &Mutex<Option<PipelineProgress>>, value: Option<PipelineProgress>) {
    *slot.lock().unwrap_or_else(|p| p.into_inner()) = value;
}

/// 读槽位（仅 Android 分发路径与单测引用；桌面构建无分发，抑制 dead_code）。
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn read_progress(slot: &Mutex<Option<PipelineProgress>>) -> Option<PipelineProgress> {
    *slot.lock().unwrap_or_else(|p| p.into_inner())
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
fn notify_one(
    service: &str,
    active: bool,
    flag: &AtomicBool,
    progress_slot: &Mutex<Option<PipelineProgress>>,
) {
    flag.store(active, Ordering::SeqCst);
    if !active {
        // 失活：清空该管线进度槽（电平数据随管线撤销，避免下批启动时
        // 通知读到上一批的残留进度）。
        write_progress(progress_slot, None);
    }
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
    // 启停边沿搭载当前进度快照：服务启动时即可渲染首帧进度，消除「首帧
    // 通知构建早于首个进度 JNI 到达」的竞态（真机实测首图无进度、次图才
    // 出现）。Kotlin 侧 start 边沿仅在快照为空时采用（floor 语义），不会
    // 覆盖可能先到的更新进度。
    let cg = read_progress(&CG_PROGRESS);
    let ai = read_progress(&AI_PROGRESS);
    crate::platform::android::sync_processing_state(combined, combine_progress_json(cg, ai));
}

#[cfg(not(target_os = "android"))]
fn dispatch_platform_sync(_combined: bool) {
    // 非 Android：无前台服务概念，纯 no-op（标志已在 notify_one 更新）。
}

/// 组合两槽位为 JSON 推给 Kotlin 刷新通知。与启停边沿共用「活跃才发」
/// 纪律（调用方已检查 SYNCED_COMBINED_ACTIVE），但**无自己的边沿检测**：
/// 进度是电平型数据，最新者胜。非 Android no-op。
#[cfg(target_os = "android")]
fn dispatch_progress_sync() {
    // 两槽位分别短临界区读取，不嵌套加锁（无锁序问题）。
    let cg = read_progress(&CG_PROGRESS);
    let ai = read_progress(&AI_PROGRESS);
    crate::platform::android::sync_processing_progress(combine_progress_json(cg, ai));
}

#[cfg(not(target_os = "android"))]
fn dispatch_progress_sync() {
    // 非 Android：无前台服务概念，纯 no-op（槽位已更新，供单测/诊断读取）。
}

/// 纯函数：组合两槽位为 `{"cg":{...}|null,"ai":{...}|null}`。
/// （仅 Android 分发路径与单测引用；桌面构建无分发，抑制 dead_code。）
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn combine_progress_json(cg: Option<PipelineProgress>, ai: Option<PipelineProgress>) -> String {
    fn one(p: Option<PipelineProgress>) -> String {
        match p {
            Some(p) => format!(
                "{{\"done\":{},\"total\":{},\"failed\":{}}}",
                p.done, p.total, p.failed
            ),
            None => "null".to_string(),
        }
    }
    format!("{{\"cg\":{},\"ai\":{}}}", one(cg), one(ai))
}

#[cfg(test)]
mod tests {
    use super::*;

    // 只测纯函数与独立的槽位读写辅助：notify_* 系列触碰全局静态且
    // dispatch 依赖平台，由 Kotlin 侧测试与 worker 集成路径覆盖。

    #[test]
    fn notify_enqueued_merges_running_batch_and_bootstraps_empty_slot() {
        // 运行中追加：保留 done/failed，total = done + depth。
        let slot = Mutex::new(Some(PipelineProgress {
            done: 2,
            total: 5,
            failed: 1,
        }));
        notify_enqueued(&slot, 6);
        let merged = read_progress(&slot).unwrap();
        assert_eq!(merged.done, 2);
        assert_eq!(merged.total, 8);
        assert_eq!(merged.failed, 1);

        // 空闲起步：{0, depth, 0}。
        let fresh = Mutex::new(None);
        notify_enqueued(&fresh, 3);
        let boot = read_progress(&fresh).unwrap();
        assert_eq!(boot.done, 0);
        assert_eq!(boot.total, 3);
        assert_eq!(boot.failed, 0);
    }

    #[test]
    fn combine_progress_json_renders_null_and_objects() {
        let cg = PipelineProgress {
            done: 1,
            total: 2,
            failed: 0,
        };
        let ai = PipelineProgress {
            done: 2,
            total: 3,
            failed: 1,
        };

        assert_eq!(
            combine_progress_json(None, None),
            r#"{"cg":null,"ai":null}"#
        );
        assert_eq!(
            combine_progress_json(Some(cg), None),
            r#"{"cg":{"done":1,"total":2,"failed":0},"ai":null}"#
        );
        assert_eq!(
            combine_progress_json(Some(cg), Some(ai)),
            r#"{"cg":{"done":1,"total":2,"failed":0},"ai":{"done":2,"total":3,"failed":1}}"#
        );
    }

    #[test]
    fn progress_slot_write_and_clear_roundtrip() {
        // 局部槽位：避免与并行 worker 测试的全局静态互相干扰。
        let slot: Mutex<Option<PipelineProgress>> = Mutex::new(None);

        assert_eq!(read_progress(&slot), None);

        let p = PipelineProgress {
            done: 3,
            total: 5,
            failed: 2,
        };
        write_progress(&slot, Some(p));
        assert_eq!(read_progress(&slot), Some(p));

        // 失活清空路径（notify_one 内部对全局槽位执行的同一辅助）。
        write_progress(&slot, None);
        assert_eq!(read_progress(&slot), None);
    }
}
