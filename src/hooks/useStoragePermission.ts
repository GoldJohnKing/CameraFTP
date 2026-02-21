import { useState, useCallback, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';

export interface StoragePathInfo {
  path_name: string;
  uri: string;
  raw_path?: string;
  is_valid: boolean;
}

export interface ServerStartCheckResult {
  can_start: boolean;
  reason?: string;
  current_path?: StoragePathInfo;
}

interface StoragePermissionState {
  pathInfo: StoragePathInfo | null;
  isLoading: boolean;
  isChecking: boolean;
  error: string | null;
}

export function useStoragePermission() {
  const [state, setState] = useState<StoragePermissionState>({
    pathInfo: null,
    isLoading: false,
    isChecking: false,
    error: null,
  });

  // Load current storage path info
  const loadStoragePath = useCallback(async () => {
    setState(prev => ({ ...prev, isLoading: true, error: null }));
    
    try {
      const info = await invoke<StoragePathInfo | null>('get_storage_path');
      setState(prev => ({
        ...prev,
        pathInfo: info,
        isLoading: false,
      }));
      return info;
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : String(err);
      setState(prev => ({
        ...prev,
        isLoading: false,
        error: errorMsg,
      }));
      return null;
    }
  }, []);

  // Check server start prerequisites
  const checkPrerequisites = useCallback(async (): Promise<ServerStartCheckResult> => {
    setState(prev => ({ ...prev, isChecking: true }));
    
    try {
      const result = await invoke<ServerStartCheckResult>('check_server_start_prerequisites');
      
      if (result.current_path) {
        setState(prev => ({
          ...prev,
          pathInfo: result.current_path || null,
          isChecking: false,
        }));
      }
      
      return result;
    } catch (err) {
      setState(prev => ({ ...prev, isChecking: false }));
      return {
        can_start: false,
        reason: err instanceof Error ? err.message : String(err),
      };
    }
  }, []);

  // Save storage path
  const saveStoragePath = useCallback(async (pathName: string, uri: string): Promise<boolean> => {
    try {
      await invoke('save_storage_path', { pathName, uri });
      await loadStoragePath();
      return true;
    } catch (err) {
      console.error('Failed to save storage path:', err);
      return false;
    }
  }, [loadStoragePath]);

  // Get last URI for picker pre-selection
  const getLastUri = useCallback(async (): Promise<string | null> => {
    try {
      const uri = await invoke<string | null>('get_last_storage_uri');
      return uri;
    } catch {
      return null;
    }
  }, []);

  // Initialize on mount
  useEffect(() => {
    loadStoragePath();
  }, [loadStoragePath]);

  return {
    ...state,
    loadStoragePath,
    checkPrerequisites,
    saveStoragePath,
    getLastUri,
  };
}
