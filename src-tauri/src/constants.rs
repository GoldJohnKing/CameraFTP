//! 应用常量定义
//!
//! 此模块包含跨平台共享的常量定义。
//! 放置在此模块可避免循环依赖问题。

/// Android 默认存储路径
/// 固定路径：/storage/emulated/0/DCIM/CameraFTP
pub const ANDROID_DEFAULT_STORAGE_PATH: &str = "/storage/emulated/0/DCIM/CameraFTP";

/// Android DCIM 目录路径（用于权限检查）
pub const ANDROID_DCIM_PATH: &str = "/storage/emulated/0/DCIM";

/// 存储路径显示名称
pub const ANDROID_STORAGE_DISPLAY_NAME: &str = "DCIM/CameraFTP";
