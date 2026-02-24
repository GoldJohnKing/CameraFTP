use super::types::{PermissionStatus, StorageInfo};
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::Mutex;

/// 平台服务接口
/// 定义各平台需要实现的统一接口
pub trait PlatformService: Send + Sync {
    /// 获取平台名称
    fn name(&self) -> &'static str;

    /// 初始化平台特定功能（托盘、权限等）
    fn setup(&self, app: &AppHandle) -> Result<(), Box<dyn std::error::Error>>;

    /// 获取存储路径信息
    fn get_storage_info(&self) -> StorageInfo;

    /// 检查权限状态
    fn check_permission_status(&self) -> PermissionStatus;

    /// 确保存储就绪
    fn ensure_storage_ready(&self) -> Result<String, String>;

    /// 服务器启动时的回调
    fn on_server_started(&self, _app: &AppHandle) {}

    /// 服务器停止时的回调
    fn on_server_stopped(&self, _app: &AppHandle) {}

    /// 更新服务器状态（用于托盘图标等）
    fn update_server_state(&self, _app: &AppHandle, _connected_clients: u32) {}

    // ========== 开机自启相关 ==========

    /// 是否支持开机自启动
    fn supports_autostart(&self) -> bool {
        false
    }

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

    // ========== 存储路径相关 ==========

    /// 获取存储路径
    fn get_storage_path(&self) -> Result<String, String>;
}
