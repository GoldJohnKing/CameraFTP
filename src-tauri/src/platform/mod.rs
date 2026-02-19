// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

mod types;
mod traits;

pub use types::{StorageInfo, PermissionStatus, ServerStartCheckResult};
pub use traits::PlatformService;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "android")]
pub mod android;

// 平台实例获取函数
#[cfg(target_os = "windows")]
pub fn get_platform() -> &'static dyn PlatformService {
    static PLATFORM: windows::WindowsPlatform = windows::WindowsPlatform;
    &PLATFORM
}

#[cfg(target_os = "android")]
pub fn get_platform() -> &'static dyn PlatformService {
    static PLATFORM: android::AndroidPlatform = android::AndroidPlatform;
    &PLATFORM
}
