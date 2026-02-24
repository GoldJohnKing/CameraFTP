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
  auto_open: boolean;
  auto_open_program: string | null;
  port: number;
  auto_select_port: boolean;
  file_extensions: string[];
}

/// 存储路径信息（与后端 StorageInfo 对应）
export interface StorageInfo {
  display_name: string;
  path: string;
  exists: boolean;
  writable: boolean;
  has_all_files_access: boolean;
}

/// 权限状态（与后端 PermissionStatus 对应）
export interface PermissionStatus {
  has_all_files_access: boolean;
  needs_user_action: boolean;
}

/// 服务器启动检查结果
export interface ServerStartCheckResult {
  can_start: boolean;
  reason?: string;
  storage_info?: StorageInfo;
}

// Android JS Bridge 接口
interface FileUploadAndroid {
  onFileUploaded: (path: string | null, size: number) => void;
}

interface SAFPickerAndroid {
  openAllFilesAccessSettings: () => void;
}

declare global {
  interface Window {
    FileUploadAndroid?: FileUploadAndroid;
    SAFPickerAndroid?: SAFPickerAndroid;
  }
}

export {};
