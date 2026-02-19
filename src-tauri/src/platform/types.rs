// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use serde::Serialize;
use ts_rs::TS;

/// 存储路径信息
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct StorageInfo {
    /// 显示名称
    pub display_name: String,
    /// 完整文件系统路径
    pub path: String,
    /// 路径是否存在
    pub exists: bool,
    /// 是否可写
    pub writable: bool,
    /// 是否有所有文件访问权限
    pub has_all_files_access: bool,
}

/// 权限状态
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct PermissionStatus {
    /// 是否有"所有文件访问权限"
    pub has_all_files_access: bool,
    /// 是否需要用户操作
    pub needs_user_action: bool,
}

/// 服务器启动检查结果
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct ServerStartCheckResult {
    pub can_start: bool,
    pub reason: Option<String>,
    pub storage_info: Option<StorageInfo>,
}
