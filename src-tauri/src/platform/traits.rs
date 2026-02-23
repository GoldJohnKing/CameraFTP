use super::types::{PermissionStatus, StorageInfo};
use tauri::AppHandle;

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
}
