export interface ServerInfo {
  is_running: boolean;
  ip: string;
  port: number;
  url: string;
  username: string;
  password_info: string;
}

export interface ServerStatus {
  is_running: boolean;
  connected_clients: number;
  files_received: number;
  bytes_received: number;
  last_file: string | null;
}

export interface AppConfig {
  save_path: string;
  port: number;
  auto_select_port: boolean;
}

/// 存储路径信息（与后端 StorageInfo 对应）
export interface StorageInfo {
  display_name: string;
  path: string;
  exists: boolean;
  writable: boolean;
}

/// 权限状态（与后端 PermissionStatus 对应）
export interface PermissionStatus {
  needs_user_action: boolean;
}

/// 服务器启动检查结果
export interface ServerStartCheckResult {
  can_start: boolean;
  reason?: string;
  storage_info?: StorageInfo;
}

// 重新导出全局类型
export * from './global';
