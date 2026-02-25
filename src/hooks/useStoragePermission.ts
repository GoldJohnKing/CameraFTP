import { useState, useCallback, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { toast } from 'sonner';

import type { StorageInfo, PermissionStatus, ServerStartCheckResult } from '../types';
import { formatError } from '../utils/error';

interface StoragePermissionState {
  storageInfo: StorageInfo | null;
  permissionStatus: PermissionStatus | null;
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
      const errorMsg = formatError(err);
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
      const errorMsg = formatError(err);
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
      const errorMsg = formatError(err);
      toast.error(errorMsg);
      return { success: false, error: errorMsg };
    }
  }, [loadStorageInfo]);

  /// 初始化
  useEffect(() => {
    loadStorageInfo();
    checkPermissionStatus();
  }, [loadStorageInfo, checkPermissionStatus]);

  return {
    ...state,
    checkPrerequisites,
    requestAllFilesPermission,
    ensureStorageReady,
    needsPermission: state.permissionStatus?.needs_user_action ?? false,
    displayName: state.storageInfo?.display_name ?? 'DCIM/CameraFTP',
  };
}
