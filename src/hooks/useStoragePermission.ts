import { useState, useCallback, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
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

  /// Android 平台权限状态自动刷新
  /// 监听从设置返回、权限对话框结果、页面可见性变化、窗口焦点变化
  /// 使用多种机制确保跨 ROM 兼容性
  useEffect(() => {
    // 仅在 Android 平台生效
    const isAndroid = typeof window !== 'undefined' && 
                      window.PermissionAndroid !== undefined;
    if (!isAndroid) return;

    let unlistenResume: UnlistenFn | undefined;
    let unlistenPermissionResult: UnlistenFn | undefined;
    let mounted = true;
    // 防抖：避免短时间内多次刷新
    let lastRefreshTime = 0;
    const DEBOUNCE_MS = 300;

    const debouncedRefresh = () => {
      const now = Date.now();
      if (now - lastRefreshTime < DEBOUNCE_MS) {
        console.log('[useStoragePermission] Debouncing refresh request');
        return;
      }
      lastRefreshTime = now;
      refreshAllPermissions();
    };

    const setupListeners = async () => {
      try {
        // 监听从设置页面返回事件（存储权限、电池优化权限）
        unlistenResume = await listen('android-on-resume', () => {
          console.log('[useStoragePermission] android-on-resume event received');
          if (mounted) {
            debouncedRefresh();
          }
        });

        // 监听系统权限对话框结果事件（通知权限）
        // 注意：此事件在某些 ROM 上可能不可靠，需要 focus 事件作为备份
        unlistenPermissionResult = await listen('android-permission-result', () => {
          console.log('[useStoragePermission] android-permission-result event received');
          if (mounted) {
            debouncedRefresh();
          }
        });

        console.log('[useStoragePermission] Event listeners setup complete');
      } catch (e) {
        console.error('[useStoragePermission] Failed to setup listeners:', e);
      }
    };

    setupListeners();

    // 监听页面可见性变化（跨ROM兼容的备份机制）
    const handleVisibilityChange = () => {
      if (document.visibilityState === 'visible') {
        console.log('[useStoragePermission] Page became visible, refreshing permissions');
        if (mounted) {
          debouncedRefresh();
        }
      }
    };

    // 监听窗口焦点变化（系统权限对话框关闭后的备份机制）
    // 这是处理系统权限对话框（如通知权限）关闭后的最可靠方式
    // 因为权限对话框是系统级别的覆盖层，不会触发 visibilitychange 或 onResume
    const handleFocus = () => {
      console.log('[useStoragePermission] Window focused, refreshing permissions');
      if (mounted) {
        debouncedRefresh();
      }
    };

    document.addEventListener('visibilitychange', handleVisibilityChange);
    window.addEventListener('focus', handleFocus);
    console.log('[useStoragePermission] Visibility and focus listeners registered');

    // 初始化时也检查一次完整权限
    checkAllAndroidPermissions();

    return () => {
      mounted = false;
      if (unlistenResume) unlistenResume();
      if (unlistenPermissionResult) unlistenPermissionResult();
      document.removeEventListener('visibilitychange', handleVisibilityChange);
      window.removeEventListener('focus', handleFocus);
      console.log('[useStoragePermission] All listeners cleaned up');
    };
  }, [refreshAllPermissions, checkAllAndroidPermissions]);

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
