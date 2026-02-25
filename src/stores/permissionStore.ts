import { create } from 'zustand';
import type { PermissionCheckResult } from '../types';
import { isPermissionAndroidAvailable, checkAndroidPermissions } from '../types';
import { formatError } from '../utils/error';

interface PermissionStoreState {
  // Permission states
  permissions: PermissionCheckResult;
  isLoading: boolean;
  error: string | null;
  isPolling: boolean;
  allGranted: boolean; // 实际状态字段，不是计算属性
  
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
  }));

// Initial check on Android only
if (typeof window !== 'undefined' && isPermissionAndroidAvailable()) {
  setTimeout(() => {
    permissionCheckInternal().then(perms => {
      if (perms) {
        usePermissionStore.getState().setPermissions(perms);
      }
    });
  }, 100);
}
