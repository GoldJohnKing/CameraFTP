// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

//! 应用常量定义
//!
//! 此模块包含跨平台共享的常量定义。
//! 放置在此模块可避免循环依赖问题。

// ============================================================================
// 存储路径常量
// ============================================================================

/// Android 默认存储路径
/// 固定路径：/storage/emulated/0/DCIM/CameraFTP
pub const ANDROID_DEFAULT_STORAGE_PATH: &str = "/storage/emulated/0/DCIM/CameraFTP";

/// Android DCIM 目录路径（用于权限检查）
pub const ANDROID_DCIM_PATH: &str = "/storage/emulated/0/DCIM";

/// 存储路径显示名称
pub const ANDROID_STORAGE_DISPLAY_NAME: &str = "DCIM/CameraFTP";

// ============================================================================
// FTP 服务器常量
// ============================================================================

/// Windows 默认 FTP 端口（使用 21 需要管理员权限）
pub const DEFAULT_FTP_PORT_WINDOWS: u16 = 21;

/// Android 默认 FTP 端口
pub const DEFAULT_FTP_PORT_ANDROID: u16 = 2121;

/// 自动端口选择的最小端口
pub const MIN_PORT: u16 = 1025;

/// 服务器启动检查的超时时间（秒）
pub const SERVER_READY_TIMEOUT_SECS: u64 = 5;

/// 服务器停止检查的超时时间（秒）
pub const SERVER_SHUTDOWN_TIMEOUT_SECS: u64 = 5;

/// 端口检查间隔（毫秒）
pub const CHECK_INTERVAL_MS: u64 = 50;

/// FTP 连接空闲超时时间（秒）
pub const IDLE_TIMEOUT_SECONDS: u64 = 600;

// ============================================================================
// 文件操作常量
// ============================================================================

/// 文件就绪检查的最大等待时间（秒）
/// 用于等待文件完全写入磁盘
pub const FILE_READY_TIMEOUT_SECS: u64 = 5;

// ============================================================================
// 预览窗口常量（Windows）
// ============================================================================

/// 预览窗口宽度
pub const PREVIEW_WINDOW_WIDTH: f64 = 1024.0;

/// 预览窗口高度
pub const PREVIEW_WINDOW_HEIGHT: f64 = 768.0;

/// 预览窗口置顶持续时间（秒）
pub const PREVIEW_ON_TOP_DURATION_SECS: u64 = 2;

/// 预览事件发送延迟（毫秒）
pub const PREVIEW_EMIT_DELAY_MS: u64 = 300;

// ============================================================================
// 自动启动常量
// ============================================================================

/// 自动启动时服务器启动延迟（毫秒）
/// 用于确保应用完全初始化后再启动服务器
pub const AUTOSTART_DELAY_MS: u64 = 500;
