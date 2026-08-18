/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import type { PermissionCheckResult } from '../types';

/**
 * 检查 Android 权限管理是否可用
 */
function isPermissionAndroidAvailable(): boolean {
  return typeof window !== 'undefined' &&
         !!window.PermissionAndroid &&
         typeof window.PermissionAndroid.checkAllPermissions === 'function';
}

/**
 * Permission bridge adapter
 * Provides a clean interface for Android permission management
 * (injected into the WebView as window.PermissionAndroid).
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
   * @returns 权限检查结果，非 Android 平台返回 null
   */
  async checkAll(): Promise<PermissionCheckResult | null> {
    if (!isPermissionAndroidAvailable()) {
      return null;
    }

    try {
      const result = await window.PermissionAndroid!.checkAllPermissions();
      return JSON.parse(result) as PermissionCheckResult;
    } catch {
      return null;
    }
  },
};
