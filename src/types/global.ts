/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

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
  
  /**
   * 打开外部链接
   * @param url 要打开的 URL
   */
  openExternalLink: (url: string) => void;
  
   /**
    * 保存图片到相册
    * @param assetPath 资源路径
    * @returns JSON 字符串，包含 success 和 message
    */
   saveImageToGallery: (assetPath: string) => Promise<string>;
   
   /**
    * 用外部APP打开图片
    * 首次点击会显示选择器，用户选择"始终"后系统会记住选择
    * @param path 图片的绝对路径
    * @returns JSON 字符串，包含 success 和 message
    */
   openImageWithChooser: (path: string) => string;
}

/**
 * Gallery image data returned by Android file scanner
 * Uses file path as unique identifier (not MediaStore ID)
 */
export interface GalleryImage {
  path: string; // 完整文件路径（作为主键）
  filename: string;
  sortTime: number; // EXIF优先的排序时间
  // thumbnail is loaded on-demand
  thumbnail?: string;
}

/**
 * Result of deleting images
 */
export interface DeleteImagesResult {
  /** Paths of successfully deleted files */
  deleted: string[];
  /** Paths of files that didn't exist (should still animate) */
  notFound: string[];
  /** Paths of files that failed to delete (should not animate) */
  failed: string[];
}

/**
 * Android Gallery interface
 * Provides access to device image gallery via direct file access
 * Uses lazy loading for thumbnails to improve performance
 */
interface GalleryAndroid {
  /**
   * Delete images by their paths
   * @param pathsJson JSON array of image paths to delete
   * @returns JSON string with deletion results containing deleted, notFound, and failed arrays
   */
  deleteImages(pathsJson: string): string | Promise<string>;

  /**
   * Remove thumbnail cache files for the given paths
   * Called after delete animation completes
   * @param pathsJson JSON array of image paths to remove thumbnails for
   * @returns true if any thumbnails were removed
   */
  removeThumbnails(pathsJson: string): boolean | Promise<boolean>;

  /**
   * Share images by their paths
   * @param pathsJson JSON array of image paths to share
   * @returns true if sharing succeeded, false otherwise
   */
  shareImages(pathsJson: string): boolean | Promise<boolean>;

  /**
   * Register back press callback to intercept back button
   * Called when entering selection mode
   * @returns true if registration succeeded
   */
  registerBackPressCallback?(): boolean;

  /**
   * Unregister back press callback
   * Called when exiting selection mode
   * @returns true if unregistration succeeded
   */
  unregisterBackPressCallback?(): boolean;

  /**
   * Callback for back button press (set by JS, called by Android)
   */
  onBackPressed?(): void;

  /**
   * List images from MediaStore
   * @returns JSON array of MediaStore entries
   */
  listMediaStoreImages(): string | Promise<string>;
}

/**
 * Android MediaStore Bridge interface
 * Optionally exposed for debug hooks
 */
interface MediaStoreAndroidBridge {
  // optionally exposed for debug hooks
}

/**
 * Android Gallery V2 Bridge interface
 * Async batched thumbnail pipeline with priority queues.
 * Injected by Android WebView as "GalleryAndroidV2".
 * All methods return JSON strings that must be parsed by the adapter layer.
 */
interface GalleryAndroidV2 {
  /**
   * List a page of media items from MediaStore
   * @param reqJson JSON string of MediaPageRequest
   * @returns JSON string of MediaPageResponse
   */
  listMediaPage(reqJson: string): Promise<string>;

  /**
   * Enqueue thumbnail generation requests
   * @param reqsJson JSON array of ThumbRequest
   * @returns JSON string (empty on success)
   */
  enqueueThumbnails(reqsJson: string): Promise<string>;

  /**
   * Cancel specific thumbnail requests by request ID
   * @param requestIdsJson JSON array of request ID strings
   * @returns JSON string (empty on success)
   */
  cancelThumbnailRequests(requestIdsJson: string): Promise<string>;

  /**
   * Cancel all thumbnail requests associated with a view
   * @param viewId The view identifier
   * @returns JSON string (empty on success)
   */
  cancelByView(viewId: string): Promise<string>;

  /**
   * Register a listener for thumbnail results
   * @param viewId The view identifier to scope results
   * @param listenerId Unique listener identifier
   * @returns JSON string (empty on success)
   */
  registerThumbnailListener(viewId: string, listenerId: string): Promise<string>;

  /**
   * Unregister a thumbnail result listener
   * @param listenerId The listener identifier to remove
   * @returns JSON string (empty on success)
   */
  unregisterThumbnailListener(listenerId: string): Promise<string>;

  /**
   * Invalidate cached thumbnails for specific media IDs
   * @param mediaIdsJson JSON array of media ID strings
   * @returns JSON string (empty on success)
   */
  invalidateMediaIds(mediaIdsJson: string): Promise<string>;

  /**
   * Get current thumbnail queue statistics
   * @returns JSON string of QueueStats
   */
  getQueueStats(): Promise<string>;
}

/**
 * Android Image Viewer Bridge interface
 * Provides built-in image viewer with zoom, pan, and swipe navigation
 */
interface ImageViewerAndroid {
  /**
   * Open the built-in image viewer
   * @param uri Content URI of the target image
   * @param allUrisJson JSON array of all image URIs for navigation
   * @returns true if viewer opened successfully
   */
  openViewer(uri: string, allUrisJson: string): boolean;

  /**
   * Close the image viewer
   * @returns true if viewer closed successfully
   */
  closeViewer(): boolean;

  /**
   * Callback from Tauri IPC when EXIF data is fetched
   * @param exifJson JSON string of ExifInfo, or null
   */
  onExifResult(exifJson: string | null): void;

  /**
   * Resolve a content:// URI to a real file system path
   * @param uri Content URI or file path
   * @returns real file path, or null if resolution fails
   */
  resolveFilePath(uri: string): string | null;
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
    
    /**
     * Android Gallery JS Bridge (legacy, will be removed in Task 12)
     */
    GalleryAndroid?: GalleryAndroid;
    
    /**
     * Android Gallery V2 JS Bridge
     * Async batched thumbnail pipeline
     */
    GalleryAndroidV2?: GalleryAndroidV2;
    
    /**
     * Android MediaStore Bridge for debug hooks
     */
    MediaStoreAndroid?: MediaStoreAndroidBridge;

    /**
     * Android Image Viewer JS Bridge
     */
    ImageViewerAndroid?: ImageViewerAndroid;

    /**
     * Global dispatch callback for V2 thumbnail results.
     * Called by the Android bridge: window.__galleryThumbDispatch(listenerId, resultJson)
     */
    __galleryThumbDispatch?: (listenerId: string, resultJson: string) => void;
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

// ===== Android Bridge Adapters =====

/**
 * Server state bridge adapter
 * Provides a clean interface for updating Android foreground service state
 */
export const serverStateBridge = {
  /**
   * Check if the server state bridge is available
   */
  isAvailable(): boolean {
    return typeof window !== 'undefined' && !!window.ServerStateAndroid;
  },

  /**
   * Update the foreground service with current server state
   */
  updateState(isRunning: boolean, statsJson: string | null, connectedClients: number): boolean {
    if (!window.ServerStateAndroid) return false;
    try {
      window.ServerStateAndroid.onServerStateChanged(isRunning, statsJson, connectedClients);
      return true;
    } catch {
      return false;
    }
  },
};

/**
 * Permission bridge adapter
 * Provides a clean interface for Android permission management
 */
export const permissionBridge = {
  /**
   * Check if the permission bridge is available
   */
  isAvailable(): boolean {
    return isPermissionAndroidAvailable();
  },

  /**
   * Request storage permission
   */
  requestStorage(): void {
    window.PermissionAndroid?.requestStoragePermission();
  },

  /**
   * Request notification permission
   */
  requestNotification(): void {
    window.PermissionAndroid?.requestNotificationPermission();
  },

  /**
   * Request battery optimization exemption
   */
  requestBatteryOptimization(): void {
    window.PermissionAndroid?.requestBatteryOptimization();
  },

  /**
   * Check all permissions
   */
  async checkAll(): Promise<PermissionCheckResult | null> {
    return checkAndroidPermissions();
  },
};

/**
 * Storage settings bridge adapter
 * Provides a clean interface for opening Android storage settings
 */
export const storageSettingsBridge = {
  /**
   * Check if the storage settings bridge is available
   */
  isAvailable(): boolean {
    return typeof window !== 'undefined' && !!window.StorageSettingsAndroid;
  },

  /**
   * Open the all files access settings page
   */
  openAllFilesAccessSettings(): void {
    window.StorageSettingsAndroid?.openAllFilesAccessSettings();
  },
};
