// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::types::{PermissionStatus, ServerStartCheckResult, StorageInfo};
use crate::ftp::types::ServerStateSnapshot;
use std::sync::Arc;
use tauri::AppHandle;
#[cfg(target_os = "windows")]
use tauri::Manager;
use tokio::sync::Mutex;

/// 平台服务接口
/// 定义各平台需要实现的统一接口
pub trait PlatformService: Send + Sync {
    /// 获取平台名称
    fn name(&self) -> &'static str;

    /// 初始化平台特定功能（托盘、权限等）
    fn setup(&self, app: &AppHandle) -> Result<(), Box<dyn std::error::Error>>;

    // ========== 存储与权限相关 ==========

    /// 获取存储路径信息
    fn get_storage_info(&self) -> StorageInfo;

    /// 检查权限状态
    fn check_permission_status(&self) -> PermissionStatus;

    /// 确保存储就绪
    fn ensure_storage_ready(&self, app: &AppHandle) -> Result<String, String>;

    /// 请求所有文件访问权限（仅 Android 有效）
    /// 返回 true 表示已授予权限，false 表示需要用户操作
    fn request_all_files_permission(&self, _app: &AppHandle) -> Result<bool, String> {
        // 默认实现：桌面平台始终返回 true
        Ok(true)
    }

    /// 检查服务器启动前提条件
    fn check_server_start_prerequisites(&self) -> ServerStartCheckResult {
        let storage_info = self.get_storage_info();
        let permission_status = self.check_permission_status();

        let can_start = storage_info.writable
            || (permission_status.has_all_files_access && !storage_info.exists);

        let reason = if !can_start {
            if !storage_info.has_all_files_access {
                Some("需要授予\"所有文件访问权限\"才能启动服务器。请在设置中开启权限。".to_string())
            } else {
                Some("存储路径不可写，请检查权限设置".to_string())
            }
        } else {
            None
        };

        ServerStartCheckResult {
            can_start,
            reason,
            storage_info: Some(storage_info),
        }
    }

    // ========== 默认存储路径 ==========

    /// 获取默认存储路径
    fn get_default_storage_path(&self) -> std::path::PathBuf;

    // ========== 服务器生命周期回调 ==========

    /// 服务器启动时的回调
    fn on_server_started(&self, _app: &AppHandle) {}

    /// 服务器停止时的回调
    fn on_server_stopped(&self, _app: &AppHandle) {}

    /// 更新服务器状态（用于托盘图标等）
    fn update_server_state(&self, _app: &AppHandle, _connected_clients: u32) {}

    /// 将 Rust 服务器状态同步到 Android 原生前台服务协调器。
    fn sync_android_service_state(&self, _app: &AppHandle, _snapshot: &ServerStateSnapshot) {}

    // ========== 开机自启相关 ==========

    /// 设置开机自启动
    fn set_autostart(&self, _enable: bool) -> Result<(), String> {
        Err(format!("开机自启在 {} 平台不支持", self.name()))
    }

    /// 检查开机自启动状态
    fn is_autostart_enabled(&self) -> Result<bool, String> {
        Ok(false)
    }

    /// 检查当前是否是开机自启模式
    fn is_autostart_mode(&self) -> bool {
        false
    }

    /// 开机自启模式下隐藏窗口
    fn hide_window_on_autostart(&self, _app: &AppHandle) {}

    /// 执行开机自启服务器启动逻辑
    /// 返回 true 表示已处理（需要等待），false 表示跳过
    fn execute_autostart_server(
        &self,
        _app: &AppHandle,
        _state: &Arc<Mutex<Option<crate::ftp::FtpServerHandle>>>,
    ) {
        // 默认实现：无操作
    }

    // ========== 窗口与UI相关 ==========

    /// 隐藏主窗口（Windows 最小化到托盘，Android 进入后台）
    fn hide_main_window(&self, _app: &AppHandle) -> Result<(), String> {
        // 默认实现：无操作
        Ok(())
    }

    /// 显示并聚焦主窗口
    fn show_main_window(&self, app: &AppHandle) -> Result<(), String> {
        #[cfg(target_os = "windows")]
        {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_skip_taskbar(false);
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
        #[cfg(target_os = "android")]
        {
            let _ = app;
        }
        Ok(())
    }

    /// 选择保存目录（Windows 打开对话框，Android 返回固定路径）
    fn select_save_directory(&self, _app: &AppHandle) -> Result<Option<String>, String> {
        // 默认实现：返回 None
        Ok(None)
    }
}
