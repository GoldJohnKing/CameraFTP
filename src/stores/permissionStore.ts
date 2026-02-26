import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { toast } from 'sonner';
import type { PermissionCheckResult, StorageInfo, PermissionStatus, ServerStartCheckResult } from '../types';
import { isPermissionAndroidAvailable, checkAndroidPermissions } from '../types';
import { formatError } from '../utils/error';

interface PermissionStoreState {
  // Permission states
  permissions: PermissionCheckResult;
  isLoading: boolean;
  error: string | null;
  isPolling: boolean;
  allGranted: boolean; // 实际状态字段，不是计算属性
  
  // Storage states (merged from useStoragePermission)
  storageInfo: StorageInfo | null;
  needsPermission: boolean;
  
  // Actions
  setPermissions: (permissions: PermissionCheckResult) => void;
  
  // Check permissions from Android
  checkPermissions: () => Promise<PermissionCheckResult>;
  
  // Request permissions (does NOT start polling)
  requestStoragePermission: () => void;
  requestNotificationPermission: () => void;
  requestBatteryOptimization: () => void;
  
  // Start/stop polling - controlled by PermissionDialog only
  startPolling: () => void;
  stopPolling: () => void;
  
  // Storage operations (merged from useStoragePermission)
  loadStorageInfo: () => Promise<StorageInfo | null>;
  checkPermissionStatus: () => Promise<PermissionStatus | null>;
  checkPrerequisites: () => Promise<ServerStartCheckResult>;
  requestAllFilesPermission: () => Promise<void>;
  ensureStorageReady: () => Promise<{ success: boolean; error?: string }>;
}

/// Internal permission check that returns default values for non-Android platforms
async function permissionCheckInternal(): Promise<PermissionCheckResult | null> {
  if (!isPermissionAndroidAvailable()) {
    return { storage: true, notification: true, batteryOptimization: true };
  }
  return checkAndroidPermissions();
}

// Helper to check if all permissions are granted
function checkAllGranted(perms: PermissionCheckResult): boolean {
  return perms.storage && perms.notification && perms.batteryOptimization;
}

// Polling state
let pollingIntervalId: number | null = null;
const POLLING_INTERVAL_MS = 200; // Poll every 200ms when active

/**
 * Permission Store using Zustand
 * Uses polling instead of events for reliability
 */
export const usePermissionStore = create<PermissionStoreState>()((set, get) => ({
    // Initial state
    permissions: {
      storage: false,
      notification: false,
      batteryOptimization: false,
    },
    isLoading: false,
    error: null,
    isPolling: false,
    allGranted: false,
    
    // Storage states
    storageInfo: null,
    needsPermission: false,
    
    // Actions - 必须传入完整对象，内部计算 allGranted
    setPermissions: (newPerms) => {
      const allGranted = checkAllGranted(newPerms);
      set({
        permissions: newPerms,
        allGranted,
      });
    },
    
    // Check permissions from Android
    checkPermissions: async () => {
      set({ isLoading: true, error: null });
      
      try {
        const perms = await permissionCheckInternal();
        
        if (perms) {
          const allGranted = checkAllGranted(perms);
          set({ 
            permissions: perms, 
            allGranted,
            isLoading: false,
          });
          return perms;
        } else {
          set({ isLoading: false, error: 'Failed to check permissions' });
          return get().permissions;
        }
      } catch (err) {
        const errorMsg = formatError(err);
        set({ isLoading: false, error: errorMsg });
        return get().permissions;
      }
    },
    
    // Request storage permission (does NOT start polling - caller must call startPolling)
    requestStoragePermission: () => {
      if (isPermissionAndroidAvailable() && window.PermissionAndroid) {
        window.PermissionAndroid.requestStoragePermission();
      }
    },
    
    // Request notification permission (does NOT start polling - caller must call startPolling)
    requestNotificationPermission: () => {
      if (isPermissionAndroidAvailable() && window.PermissionAndroid) {
        window.PermissionAndroid.requestNotificationPermission();
      }
    },
    
    // Request battery optimization (does NOT start polling - caller must call startPolling)
    requestBatteryOptimization: () => {
      if (isPermissionAndroidAvailable() && window.PermissionAndroid) {
        window.PermissionAndroid.requestBatteryOptimization();
      }
    },
    
    // Start polling for permission changes
    // Only PermissionDialog should call this
    startPolling: () => {
      if (!isPermissionAndroidAvailable()) return;
      
      // Stop existing polling first
      if (pollingIntervalId !== null) {
        window.clearInterval(pollingIntervalId);
      }
      
      set({ isPolling: true });
      
      // Store previous state to detect changes
      let previousState = { ...get().permissions };
      let stopPollingRequested = false;
      
      // Check immediately
      permissionCheckInternal().then(perms => {
        if (perms) {
          previousState = perms;
          get().setPermissions(perms);
          
          // If all already granted, stop immediately
          if (perms.storage && perms.notification && perms.batteryOptimization) {
            get().stopPolling();
            return;
          }
        }
      });
      
      // Start interval
      pollingIntervalId = window.setInterval(async () => {
        // Skip if stop was requested
        if (stopPollingRequested) return;
        
        const perms = await permissionCheckInternal();
        if (perms) {
          // Check if anything changed
          const hasChanged = 
            perms.storage !== previousState.storage ||
            perms.notification !== previousState.notification ||
            perms.batteryOptimization !== previousState.batteryOptimization;
          
          if (hasChanged) {
            previousState = perms;
            get().setPermissions(perms);
          }
          
          // If all granted, request stop (but let this interval finish)
          if (perms.storage && perms.notification && perms.batteryOptimization) {
            stopPollingRequested = true;
            // Delay stop to ensure state is propagated
            window.setTimeout(() => {
              get().stopPolling();
            }, 100);
          }
        }
      }, POLLING_INTERVAL_MS);
    },
    
    // Stop polling
    stopPolling: () => {
      if (pollingIntervalId !== null) {
        window.clearInterval(pollingIntervalId);
        pollingIntervalId = null;
      }
      set({ isPolling: false });
    },
    
    // === Storage operations (merged from useStoragePermission) ===
    
    // Load storage info
    loadStorageInfo: async () => {
      set({ isLoading: true });
      
      try {
        const info = await invoke<StorageInfo>('get_storage_info');
        set({
          storageInfo: info,
          isLoading: false,
        });
        return info;
      } catch (err) {
        const errorMsg = formatError(err);
        toast.error(errorMsg);
        set({ isLoading: false });
        return null;
      }
    },
    
    // Check permission status
    checkPermissionStatus: async () => {
      try {
        const status = await invoke<PermissionStatus>('check_permission_status');
        set({ needsPermission: status.needs_user_action });
        return status;
      } catch {
        return null;
      }
    },
    
    // Check server start prerequisites
    checkPrerequisites: async () => {
      try {
        const result = await invoke<ServerStartCheckResult>('check_server_start_prerequisites');
        
        if (result.storage_info) {
          set({ storageInfo: result.storage_info });
        }
        
        return result;
      } catch (err) {
        const errorMsg = formatError(err);
        return {
          can_start: false,
          reason: errorMsg,
        };
      }
    },
    
    // Request all files permission (opens system settings)
    requestAllFilesPermission: async () => {
      try {
        await invoke('request_all_files_permission');
      } catch {
        toast.error('无法打开设置页面');
      }
    },
    
    // Ensure storage is ready
    ensureStorageReady: async () => {
      try {
        await invoke<string>('ensure_storage_ready');
        await get().loadStorageInfo();
        return { success: true };
      } catch (err) {
        const errorMsg = formatError(err);
        toast.error(errorMsg);
        return { success: false, error: errorMsg };
      }
    },
  }));

// Initial check on Android only
if (typeof window !== 'undefined' && isPermissionAndroidAvailable()) {
  setTimeout(() => {
    const state = usePermissionStore.getState();
    // Check permissions
    permissionCheckInternal().then(perms => {
      if (perms) {
        state.setPermissions(perms);
      }
    });
    // Load storage info and check permission status
    state.loadStorageInfo();
    state.checkPermissionStatus();
  }, 100);
}
