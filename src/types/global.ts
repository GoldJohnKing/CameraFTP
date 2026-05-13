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
    * 用外部APP打开图片
    * 首次点击会显示选择器，用户选择"始终"后系统会记住选择
    * @param path 图片的 content:// URI 或文件路径
    * @returns JSON 字符串，包含 success 和 message
    */
   openImageWithChooser: (path: string) => string;
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
 * Legacy MediaStore URI bridge for gallery actions
 * Operates on content URI arrays serialized as JSON strings
 */
interface GalleryAndroid {
  /**
   * Delete images by their content URIs
   * @param urisJson JSON array of image content URIs to delete
   * @returns JSON string with deletion results containing deleted, notFound, and failed arrays
   */
  deleteImages(urisJson: string): string | Promise<string>;

  /**
   * Share images by their content URIs
   * @param urisJson JSON array of image content URIs to share
   * @returns true if sharing succeeded, false otherwise
   */
  shareImages(urisJson: string): boolean | Promise<boolean>;

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

}

/**
 * Android Image Viewer Bridge interface
 * Provides built-in image viewer with zoom, pan, and swipe navigation
 */
interface ImageViewerAndroid {
  /**
   * Reuse existing viewer if visible, otherwise open viewer
   * @param uri Content URI of the target image
   * @param allUrisJson JSON array of all image URIs for navigation
   * @returns true if navigation/open action succeeded
   */
  openOrNavigateTo(uri: string, allUrisJson: string): boolean;

  /**
   * Check whether image viewer app/activity is currently visible
   * @returns true when app is visible to user
   */
  isAppVisible(): boolean;

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

  /**
   * Callback from Tauri IPC when EXIF data is fetched for a specific position
   * @param position Adapter position of the image
   * @param exifJson JSON string of ExifInfo, or null
   */
  onExifResultForPosition(position: number, exifJson: string | null): void;

  /**
   * Request batch EXIF resolution for multiple image positions.
   * The native viewer calls this to prefetch EXIF for offscreen pages.
   * For each item, onExifResultForPosition will be called with the result.
   * @param requestJson JSON array of { position: number, uri: string }
   */
  requestExifForPositions(requestJson: string): void;

  /**
   * Callback from JS when an AI edit triggered from native viewer completes
   * @param success Whether the edit succeeded
   * @param message Status message, or null if cancelled
   * @param cancelled Whether the edit was cancelled by user
   */
  onAiEditComplete?(success: boolean, message: string | null, cancelled: boolean): void;

  /**
   * Update AI edit progress in native UI
   * @param current Current file index being processed
   * @param total Total number of files to process
   * @param failedCount Number of files that failed so far
   */
  updateAiEditProgress?(current: number, total: number, failedCount: number): void;

  /**
   * Triggers a MediaStore scan for a newly created file so it appears in the system gallery.
   * @param filePath Absolute file path to scan
   */
  scanNewFile?(filePath: string): void;

  /**
   * Insert a new image into the currently visible viewer at a specific position.
   * No-op if the viewer is not visible or URI already exists in the list.
   * @param uri Content URI of the new image
   * @param insertIndex Position to insert at (clamped to valid range)
   * @returns true if inserted into an active viewer
   */
  insertImage?(uri: string, insertIndex: number): boolean;

  /**
   * Navigate the active viewer to an existing URI already in its list.
   * No-op if the viewer is not visible or URI is not in the list.
   * @param uri Content URI to navigate to
   */
  navigateToExistingUri?(uri: string): void;

   /**
    * Callback from JS when a color grading triggered from native viewer completes
   * @param success Whether the grading succeeded
   * @param message Status message, or null if cancelled
   * @param cancelled Whether the grading was cancelled by user
   */
  onColorGradingComplete?(success: boolean, message: string | null, cancelled: boolean): void;

  /**
   * Update color grading progress in native UI
   * @param current Current file index being processed
   * @param total Total number of files to process
   * @param failedCount Number of files that failed so far
   */
  updateColorGradingProgress?(current: number, total: number, failedCount: number): void;
}

// ===== 全局窗口扩展 =====

declare global {
  interface Window {
    /**
     * Android 权限管理 JS Bridge
     */
    PermissionAndroid?: PermissionAndroid;
    
    /**
     * Android Gallery JS Bridge (legacy compatibility)
     */
    GalleryAndroid?: GalleryAndroid;
    
    /**
     * Android Gallery V2 JS Bridge
     * Async batched thumbnail pipeline
     */
    GalleryAndroidV2?: GalleryAndroidV2;
    
    /**
     * Android Image Viewer JS Bridge
     */
    ImageViewerAndroid?: ImageViewerAndroid;

    /**
     * Global dispatch callback for V2 thumbnail results.
     * Called by the Android bridge: window.__galleryThumbDispatch(listenerId, resultJson)
     */
    __galleryThumbDispatch?: (listenerId: string, resultJson: string) => void;

    /**
     * Global callback for Android back press handling.
     * Set by JS and invoked by Android (not exposed as a bridge instance method).
     */
    __galleryOnBackPressed?: () => void;

    /**
     * Returns the current AI edit prompt and model from config store as JSON.
     * Called by native ImageViewerActivity to pre-fill the prompt dialog.
     */
    __tauriGetAiEditPrompt?: () => string;

    /**
     * Triggers AI edit with a specific prompt, optionally saving it to config.
     * Called by native ImageViewerActivity after user confirms the prompt dialog.
     */
    __tauriTriggerAiEditWithPrompt?: (filePath: string, prompt: string, model?: string, saveAsAutoEdit?: boolean, apiKey?: string) => Promise<void>;

    /**
     * Cancels the in-progress AI edit batch.
     * Called by native ImageViewerActivity when the user taps the cancel button on the progress bar.
     */
    __tauriCancelAiEdit?: () => Promise<void>;

    /**
     * Returns the current AI edit progress state.
     * Called by native ImageViewerActivity to sync progress when opening mid-edit.
     */
    __tauriGetAiEditProgress?: () => {
      isEditing: boolean;
      isDone: boolean;
      current: number;
      total: number;
      currentFileName: string;
      failedCount: number;
      failedFiles: string[];
    };

    /**
     * Triggers color grading for a single file.
     * Called by native ImageViewerActivity after user confirms the color grading dialog.
     */
    __tauriTriggerColorGrading?: (filePath: string, lutId: string, useAutoExposure: boolean, meteringMode: string, manualEv: number, syncToAuto: boolean) => Promise<void>;

    /**
     * Returns whether auto color grading is enabled as a string 'true'/'false'.
     * Called by native ImageViewerActivity to determine dialog state.
     */
    __tauriGetAutoColorGradingEnabled?: () => string;

    /**
     * Returns the last-used color grading parameters as a JSON string.
     * Called by native ImageViewerActivity to pre-fill the color grading dialog.
     */
    __tauriGetColorGradingLastUsed?: () => string;

    /**
     * Returns the color grading presets as a JSON string ([[id, displayName], ...]).
     * Called by native ImageViewerActivity to populate the preset dropdown.
     */
    __tauriGetColorGradingPresets?: () => string;

    /**
     * Cancels the in-progress color grading batch.
     * Called by native ImageViewerActivity when the user taps the cancel button on the progress bar.
     */
    __tauriCancelColorGrading?: () => Promise<void>;

    /**
     * Returns the current color grading progress state.
     * Called by native ImageViewerActivity to sync progress when opening mid-grading.
     */
    __tauriGetColorGradingProgress?: () => {
      isProcessing: boolean;
      isDone: boolean;
      current: number;
      total: number;
      currentFileName: string;
      failedCount: number;
      failedFiles: string[];
    };

    /**
     * Global handler for EXIF prefetch requests from native viewer.
     * Called via evaluateJavascript from ImageViewerActivity.
     * @param requestJson JSON array of { position: number, uri: string }
     */
    __requestExifForPositions?: (requestJson: string) => void;
  }
}

// ===== 类型守卫函数 =====

/**
 * 检查 Android 权限管理是否可用
 */
function isPermissionAndroidAvailable(): boolean {
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
