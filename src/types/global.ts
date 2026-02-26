/**
 * 全局类型定义
 * 包含窗口扩展类型和 JS Bridge 类型
 */

// ===== Android JS Bridge 类型 =====

/**
 * Android 文件上传回调接口
 * 由 Android WebView 注入
 */
interface FileUploadAndroid {
  /**
   * 文件上传完成回调
   * @param path 文件路径
   * @param size 文件大小（字节）
   */
  onFileUploaded: (path: string, size: number) => void;
}

/**
 * Android 存储权限设置接口
 * 由 Android WebView 注入
 */
interface StorageSettingsAndroid {
  /**
   * 打开"所有文件访问权限"设置页面
   */
  openAllFilesAccessSettings: () => void;
}

/**
 * Android Server State Bridge 接口
 * 用于与前台的 FTP 服务通信
 * 由 ServerStateBridge 注入为 "ServerStateAndroid"
 */
interface ServerStateAndroid {
  /**
   * 更新前台服务的状态
   * @param isRunning 服务器是否运行中
   * @param statsJson 统计信息的 JSON 字符串，或 null
   * @param connectedClients 当前连接的客户端数量
   */
  onServerStateChanged(isRunning: boolean, statsJson: string | null, connectedClients: number): void;
}

/**
 * Android 权限检查结果
 */
export interface PermissionCheckResult {
  storage: boolean;
  notification: boolean;
  batteryOptimization: boolean;
}

/**
 * Android 权限管理接口
 * 由 Android WebView 注入
 */
interface PermissionAndroid {
  /**
   * 检查所有必要权限的状态
   * @returns JSON 字符串，包含 storage, notification, batteryOptimization 的布尔值
   */
  checkAllPermissions: () => Promise<string>;
  
  /**
   * 请求存储权限
   */
  requestStoragePermission: () => void;
  
  /**
   * 请求通知权限
   */
  requestNotificationPermission: () => void;
  
  /**
   * 请求电池优化白名单
   */
  requestBatteryOptimization: () => void;
}

// ===== 全局窗口扩展 =====

declare global {
  interface Window {
    /**
     * Android 文件上传 JS Bridge
     */
    FileUploadAndroid?: FileUploadAndroid;
    
    /**
     * Android 存储权限设置 JS Bridge
     */
    StorageSettingsAndroid?: StorageSettingsAndroid;
    
    /**
     * Android Server State JS Bridge
     * 用于与前台 FTP 服务通信
     */
    ServerStateAndroid?: ServerStateAndroid;
    
    /**
     * Android 权限管理 JS Bridge
     */
    PermissionAndroid?: PermissionAndroid;
  }
}

// ===== 类型守卫函数 =====

/**
 * 检查 Android 权限管理是否可用
 */
export function isPermissionAndroidAvailable(): boolean {
  return typeof window !== 'undefined' && 
         !!window.PermissionAndroid && 
         typeof window.PermissionAndroid.checkAllPermissions === 'function';
}

/**
 * 检查 Android 权限状态
 * @returns 权限检查结果，非 Android 平台返回 null
 */
export async function checkAndroidPermissions(): Promise<PermissionCheckResult | null> {
  if (!isPermissionAndroidAvailable()) {
    return null;
  }
  
  try {
    const result = await window.PermissionAndroid!.checkAllPermissions();
    return JSON.parse(result) as PermissionCheckResult;
  } catch {
    return null;
  }
}
