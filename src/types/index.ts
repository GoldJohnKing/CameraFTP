export interface ServerInfo {
  isRunning: boolean;
  ip: string;
  port: number;
  url: string;
  username: string;
  passwordInfo: string;
}

export interface ServerStatus {
  isRunning: boolean;
  connectedClients: number;
  filesReceived: number;
  bytesReceived: number;
  lastFile: string | null;
}

export interface AuthConfig {
  anonymous: boolean;
  username: string;
  passwordHash: string;
  passwordSalt: string;
}

export interface PasvConfig {
  portStart: number;
  portEnd: number;
}

export interface AdvancedConnectionConfig {
  enabled: boolean;
  auth: AuthConfig;
  pasv: PasvConfig;
}

export interface AppConfig {
  savePath: string;
  port: number;
  autoSelectPort: boolean;
  advancedConnection: AdvancedConnectionConfig;
}

/// 存储路径信息（与后端 StorageInfo 对应）
export interface StorageInfo {
  displayName: string;
  path: string;
  exists: boolean;
  writable: boolean;
  hasAllFilesAccess: boolean;
}

/// 权限状态（与后端 PermissionStatus 对应）
export interface PermissionStatus {
  hasAllFilesAccess: boolean;
  needsUserAction: boolean;
}

/// 服务器启动检查结果
export interface ServerStartCheckResult {
  canStart: boolean;
  reason?: string;
  storageInfo?: StorageInfo;
}

/// 文件信息（用于文件浏览）
export interface FileInfo {
  path: string;
  filename: string;
}

/// EXIF 信息（用于图片元数据）
export interface ExifInfo {
  iso?: number;
  aperture?: string;      // f/2.8
  shutterSpeed?: string;  // 1/125s
  focalLength?: string;   // 24mm
  datetime?: string;      // 2024-02-27 14:30:00
}

// 重新导出全局类型
export * from './global';

// 图片打开方式
export type ImageOpenMethod =
  | 'built-in-preview'
  | 'system-default'
  | 'windows-photos'
  | 'custom';

// 预览窗口配置
export interface PreviewWindowConfig {
  enabled: boolean;
  method: ImageOpenMethod;
  customPath?: string;
  autoBringToFront: boolean;
}
