import { useState, useCallback, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { toast } from 'sonner';

// 从统一的类型文件导入
import type { StorageInfo, PermissionStatus, ServerStartCheckResult, PermissionCheckResult } from '../types';

interface StoragePermissionState {
  storageInfo: StorageInfo | null;
  permissionStatus: PermissionStatus | null;
  /// Android 完整权限状态（存储、通知、电池优化）
  androidPermissions: PermissionCheckResult | null;
  isLoading: boolean;
  error: string | null;
}

/**
 * 存储权限管理 Hook
 * 
 * 用于 Android 平台的存储权限管理。
 * - 存储路径固定为 DCIM/CameraFTP
 * - 用户只需授予"所有文件访问权限"
 */
export function useStoragePermission() {
  const [state, setState] = useState<StoragePermissionState>({
    storageInfo: null,
    permissionStatus: null,
    androidPermissions: null,
    isLoading: false,
    error: null,
  });

  const mountedRef = useRef(true);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  /// 加载存储信息
  const loadStorageInfo = useCallback(async () => {
    setState(prev => ({ ...prev, isLoading: true, error: null }));
    
    try {
      const info = await invoke<StorageInfo>('get_storage_info');
      if (!mountedRef.current) return null;
      
      setState(prev => ({
        ...prev,
        storageInfo: info,
        isLoading: false,
      }));
      return info;
    } catch (err) {
      if (!mountedRef.current) return null;
      const errorMsg = err instanceof Error ? err.message : String(err);
      setState(prev => ({
        ...prev,
        isLoading: false,
        error: errorMsg,
      }));
      return null;
    }
  }, []);

  /// 检查权限状态
  const checkPermissionStatus = useCallback(async () => {
    try {
      const status = await invoke<PermissionStatus>('check_permission_status');
      if (mountedRef.current) {
        setState(prev => ({ ...prev, permissionStatus: status }));
      }
      return status;
    } catch (err) {
      console.error('Failed to check permission status:', err);
      return null;
    }
  }, []);

  /// 检查服务器启动前提条件
  const checkPrerequisites = useCallback(async (): Promise<ServerStartCheckResult> => {
    try {
      const result = await invoke<ServerStartCheckResult>('check_server_start_prerequisites');
      
      if (mountedRef.current && result.storage_info) {
        setState(prev => ({
          ...prev,
          storageInfo: result.storage_info || null,
        }));
      }
      
      return result;
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : String(err);
      return {
        can_start: false,
        reason: errorMsg,
      };
    }
  }, []);

  /// 请求所有文件访问权限
  /// 会直接跳转到系统设置中的权限开关页面
  const requestAllFilesPermission = useCallback(async () => {
    try {
      await invoke('request_all_files_permission');
    } catch (err) {
      console.error('Failed to request permission:', err);
      toast.error('无法打开设置页面');
    }
  }, []);

  /// 确保存储就绪
  const ensureStorageReady = useCallback(async (): Promise<{ success: boolean; error?: string }> => {
    try {
      await invoke<string>('ensure_storage_ready');
      await loadStorageInfo();
      return { success: true };
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : String(err);
      toast.error(errorMsg);
      return { success: false, error: errorMsg };
    }
  }, [loadStorageInfo]);

  /// 检查存储权限
  const checkStoragePermission = useCallback(async (): Promise<boolean> => {
    try {
      return await invoke<boolean>('check_storage_permission');
    } catch {
      return false;
    }
  }, []);

  /// 检查所有 Android 权限（存储、通知、电池优化）
  /// 通过 Kotlin Bridge 调用，比 Rust 命令更全面
  const checkAllAndroidPermissions = useCallback(async (): Promise<PermissionCheckResult | null> => {
    // 仅在 Android 平台生效
    if (typeof window === 'undefined' || !window.PermissionAndroid) {
      return null;
    }

    try {
      console.log('[useStoragePermission] Checking all Android permissions...');
      const result = await window.PermissionAndroid.checkAllPermissions();
      const parsed: PermissionCheckResult = JSON.parse(result);
      console.log('[useStoragePermission] Android permissions:', parsed);
      
      if (mountedRef.current) {
        setState(prev => ({ ...prev, androidPermissions: parsed }));
      }
      return parsed;
    } catch (err) {
      console.error('[useStoragePermission] Failed to check Android permissions:', err);
      return null;
    }
  }, []);

  /// 刷新所有权限状态（统一入口）
  /// 同时调用 Rust 命令和 Kotlin Bridge，确保所有权限状态都能正确更新
  const refreshAllPermissions = useCallback(async () => {
    console.log('[useStoragePermission] Refreshing all permissions...');
    
    // 并行调用两种检查方式
    await Promise.all([
      checkPermissionStatus(),
      loadStorageInfo(),
      checkAllAndroidPermissions(),
    ]);
  }, [checkPermissionStatus, loadStorageInfo, checkAllAndroidPermissions]);

  /// 初始化
  useEffect(() => {
    loadStorageInfo();
    checkPermissionStatus();
  }, [loadStorageInfo, checkPermissionStatus]);

  // 注意：Config界面不再使用自动刷新机制
  // 权限状态在组件挂载时检查一次，之后依赖用户手动刷新
  // PermissionDialog 使用轮询机制实时检测权限变化

  return {
    ...state,
    loadStorageInfo,
    checkPermissionStatus,
    checkPrerequisites,
    requestAllFilesPermission,
    ensureStorageReady,
    checkStoragePermission,
    checkAllAndroidPermissions,
    refreshAllPermissions,
    
    /// 便捷属性
    isReady: state.storageInfo?.writable ?? false,
    needsPermission: state.permissionStatus?.needs_user_action ?? false,
    displayName: state.storageInfo?.display_name ?? 'DCIM/CameraFTP',
    storagePath: state.storageInfo?.path ?? '',
    
    /// Android 完整权限便捷属性
    hasStoragePermission: state.androidPermissions?.storage ?? false,
    hasNotificationPermission: state.androidPermissions?.notification ?? false,
    hasBatteryOptimizationPermission: state.androidPermissions?.batteryOptimization ?? false,
  };
}
