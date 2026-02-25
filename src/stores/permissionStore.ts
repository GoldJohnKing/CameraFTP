import { create } from 'zustand';
import { subscribeWithSelector } from 'zustand/middleware';
import type { PermissionCheckResult } from '../types/global';

// Window.PermissionAndroid 类型已在 global.ts 中声明，无需重复

export interface PermissionStoreState {
  // Permission states
  permissions: PermissionCheckResult;
  isLoading: boolean;
  lastCheckedAt: number | null;
  error: string | null;
  isPolling: boolean;
  allGranted: boolean; // 实际状态字段，不是计算属性
  
  // Actions
  setPermissions: (permissions: PermissionCheckResult) => void;
  setLoading: (loading: boolean) => void;
  setPolling: (polling: boolean) => void;
  
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

// Check if we're on Android platform
function isAndroid(): boolean {
  return typeof window !== 'undefined' && 
         window.PermissionAndroid !== undefined &&
         typeof window.PermissionAndroid?.checkAllPermissions === 'function';
}

// Internal permission check function
async function permissionCheckInternal(): Promise<PermissionCheckResult | null> {
  if (!isAndroid()) {
    return { storage: true, notification: true, batteryOptimization: true };
  }
  
  try {
    const result = await window.PermissionAndroid!.checkAllPermissions();
    const parsed: PermissionCheckResult = JSON.parse(result);
    return parsed;
  } catch (err) {
    console.error('[PermissionStore] Failed to check permissions:', err);
    return null;
  }
}

// Helper to check if all permissions are granted
function checkAllGranted(perms: PermissionCheckResult): boolean {
  return perms.storage && perms.notification && perms.batteryOptimization;
}

// Polling state
let pollingIntervalId: number | null = null;
const POLLING_INTERVAL_MS = 200; // Poll every 200ms when active

/**
 * Permission Store using Zustand with selector subscription
 * Uses polling instead of events for reliability
 */
export const usePermissionStore = create<PermissionStoreState>()(
  subscribeWithSelector((set, get) => ({
    // Initial state
    permissions: {
      storage: false,
      notification: false,
      batteryOptimization: false,
    },
    isLoading: false,
    lastCheckedAt: null,
    error: null,
    isPolling: false,
    allGranted: false,
    
    // Actions - 必须传入完整对象，内部计算 allGranted
    setPermissions: (newPerms) => {
      const allGranted = checkAllGranted(newPerms);
      console.log('[PermissionStore] Setting permissions:', newPerms, 'allGranted:', allGranted);
      set({
        permissions: newPerms,
        allGranted,
        lastCheckedAt: Date.now(),
      });
    },
    
    setLoading: (loading) => set({ isLoading: loading }),
    
    setPolling: (polling) => set({ isPolling: polling }),
    
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
            lastCheckedAt: Date.now(),
          });
          return perms;
        } else {
          set({ isLoading: false, error: 'Failed to check permissions' });
          return get().permissions;
        }
      } catch (err) {
        const errorMsg = err instanceof Error ? err.message : String(err);
        set({ isLoading: false, error: errorMsg });
        return get().permissions;
      }
    },
    
    // Request storage permission (does NOT start polling - caller must call startPolling)
    requestStoragePermission: () => {
      if (isAndroid() && window.PermissionAndroid) {
        console.log('[PermissionStore] Requesting storage permission');
        window.PermissionAndroid.requestStoragePermission();
      }
    },
    
    // Request notification permission (does NOT start polling - caller must call startPolling)
    requestNotificationPermission: () => {
      if (isAndroid() && window.PermissionAndroid) {
        console.log('[PermissionStore] Requesting notification permission');
        window.PermissionAndroid.requestNotificationPermission();
      }
    },
    
    // Request battery optimization (does NOT start polling - caller must call startPolling)
    requestBatteryOptimization: () => {
      if (isAndroid() && window.PermissionAndroid) {
        console.log('[PermissionStore] Requesting battery optimization');
        window.PermissionAndroid.requestBatteryOptimization();
      }
    },
    
    // Start polling for permission changes
    // Only PermissionDialog should call this
    startPolling: () => {
      if (!isAndroid()) return;
      
      // Stop existing polling first
      if (pollingIntervalId !== null) {
        window.clearInterval(pollingIntervalId);
      }
      
      console.log('[PermissionStore] Starting permission polling');
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
            console.log('[PermissionStore] All permissions already granted, stopping polling');
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
            console.log('[PermissionStore] Permission change detected:', perms);
            previousState = perms;
            get().setPermissions(perms);
          }
          
          // If all granted, request stop (but let this interval finish)
          if (perms.storage && perms.notification && perms.batteryOptimization) {
            console.log('[PermissionStore] All permissions granted, will stop polling');
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
        console.log('[PermissionStore] Stopping permission polling');
        window.clearInterval(pollingIntervalId);
        pollingIntervalId = null;
      }
      set({ isPolling: false });
    },
  }))
);

// Initial check on Android only
if (typeof window !== 'undefined' && isAndroid()) {
  setTimeout(() => {
    permissionCheckInternal().then(perms => {
      if (perms) {
        usePermissionStore.getState().setPermissions(perms);
      }
    });
  }, 100);
}

// Window.PermissionAndroid 类型已在 global.ts 中声明
