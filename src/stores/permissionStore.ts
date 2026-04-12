/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { toast } from 'sonner';
import type { PermissionCheckResult, StorageInfo, PermissionStatus, ServerStartCheckResult } from '../types';
import { permissionBridge } from '../types';
import { formatError, silent } from '../utils/error';
import { requestMediaLibraryRefresh } from '../utils/gallery-refresh';

interface PermissionStoreState {
  // Permission states
  permissions: PermissionCheckResult;
  isLoading: boolean;
  error: string | null;
  isPolling: boolean;
  allGranted: boolean; // 实际状态字段，不是计算属性
  isInitialized: boolean; // Track if store has been initialized
  
  // Storage states (merged from useStoragePermission)
  storageInfo: StorageInfo | null;
  needsPermission: boolean;
  
  // Actions
  setPermissions: (permissions: PermissionCheckResult) => void;
  initialize: () => void; // Initialize store - call once on app start
  
  // Check permissions from Android
  checkPermissions: () => Promise<PermissionCheckResult>;
  
  // Request permissions (does NOT start polling)
  requestStoragePermission: () => void;
  requestNotificationPermission: () => void;
  requestBatteryOptimization: () => void;
  
  // Start/stop polling - controlled by PermissionDialog only
  startPolling: (mode?: 'all' | 'storage') => void;
  stopPolling: () => void;
  
  // Polling interval ID (stored in state instead of module scope)
  pollingIntervalId: number | null;
  
  // Storage operations (merged from useStoragePermission)
  loadStorageInfo: () => Promise<StorageInfo | null>;
  checkPermissionStatus: () => Promise<PermissionStatus | null>;
  checkPrerequisites: () => Promise<ServerStartCheckResult>;
  requestAllFilesPermission: () => Promise<void>;
  ensureStorageReady: () => Promise<{ success: boolean; error?: string }>;
}

/// Internal permission check that returns default values for non-Android platforms
async function permissionCheckInternal(): Promise<PermissionCheckResult | null> {
  if (!permissionBridge.isAvailable()) {
    return { storage: true, notification: true, batteryOptimization: true };
  }
  return permissionBridge.checkAll();
}

// Helper to check if all permissions are granted
function checkAllGranted(perms: PermissionCheckResult): boolean {
  return perms.storage && perms.notification && perms.batteryOptimization;
}

const POLLING_INTERVAL_MS = 200; // Poll every 200ms when active
const POLLING_TIMEOUT_MS = 30000;

function shouldStopPolling(mode: 'all' | 'storage', perms: PermissionCheckResult): boolean {
  return mode === 'storage'
    ? perms.storage
    : perms.storage && perms.notification && perms.batteryOptimization;
}

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
    isInitialized: false,
    
    // Storage states
    storageInfo: null,
    needsPermission: false,
    
    // Polling state (moved from module scope)
    pollingIntervalId: null,
    
    // Actions - 必须传入完整对象，内部计算 allGranted
    setPermissions: (newPerms) => {
      const allGranted = checkAllGranted(newPerms);
      set({
        permissions: newPerms,
        allGranted,
      });
    },
    
    // Initialize store - call once on app start (Android only)
    initialize: () => {
      if (get().isInitialized) return;
      if (!permissionBridge.isAvailable()) return;
      
      set({ isInitialized: true });
      
      // Check permissions
      permissionCheckInternal().then(perms => {
        if (perms) {
          get().setPermissions(perms);
        }
      });
      // Load storage info and check permission status
      get().loadStorageInfo();
      get().checkPermissionStatus();
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
      permissionBridge.requestStorage();
    },
    
    // Request notification permission (does NOT start polling - caller must call startPolling)
    requestNotificationPermission: () => {
      permissionBridge.requestNotification();
    },
    
    // Request battery optimization (does NOT start polling - caller must call startPolling)
    requestBatteryOptimization: () => {
      permissionBridge.requestBatteryOptimization();
    },
    
    // Start polling for permission changes
    // Only PermissionDialog should call this
    startPolling: (mode = 'all') => {
      if (!permissionBridge.isAvailable()) return;
      
      const state = get();
      
      // Stop existing polling first
      if (state.pollingIntervalId !== null) {
        window.clearInterval(state.pollingIntervalId);
      }
      
      set({ isPolling: true });
      
      // Store previous state to detect changes
      let previousState = { ...state.permissions };
      let stopPollingRequested = false;
      const pollingStartedAt = Date.now();
      
      // Check immediately
      permissionCheckInternal().then(perms => {
        if (perms) {
          previousState = perms;
          get().setPermissions(perms);
          
          if (shouldStopPolling(mode, perms)) {
            get().stopPolling();
            return;
          }
        }
      });
      
      // Start interval
      const intervalId = window.setInterval(async () => {
        // Skip if stop was requested
        if (stopPollingRequested) return;

        if (Date.now() - pollingStartedAt > POLLING_TIMEOUT_MS) {
          stopPollingRequested = true;
          get().stopPolling();
          return;
        }
        
        const perms = await permissionCheckInternal();
        if (perms) {
          // Check if anything changed
          const hasChanged = 
            perms.storage !== previousState.storage ||
            perms.notification !== previousState.notification ||
            perms.batteryOptimization !== previousState.batteryOptimization;
          
          if (hasChanged) {
            const storageJustGranted = !previousState.storage && perms.storage;
            previousState = perms;
            get().setPermissions(perms);

            if (storageJustGranted) {
              requestMediaLibraryRefresh({ reason: 'permission-granted' });
            }
          }
          
          if (shouldStopPolling(mode, perms)) {
            stopPollingRequested = true;
            // Delay stop to ensure state is propagated
            window.setTimeout(() => {
              get().stopPolling();
            }, 100);
          }
        }
      }, POLLING_INTERVAL_MS);
      
      set({ pollingIntervalId: intervalId });
    },
    
    // Stop polling
    stopPolling: () => {
      const { pollingIntervalId } = get();
      if (pollingIntervalId !== null) {
        window.clearInterval(pollingIntervalId);
      }
      set({ isPolling: false, pollingIntervalId: null });
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
      return silent(async () => {
        const status = await invoke<PermissionStatus>('check_permission_status');
        set({ needsPermission: status.needsUserAction });
        return status;
      });
    },
    
    // Check server start prerequisites
    checkPrerequisites: async () => {
      try {
        const result = await invoke<ServerStartCheckResult>('check_server_start_prerequisites');
        
        if (result.storageInfo) {
          set({ storageInfo: result.storageInfo });
        }
        
        return result;
      } catch (err) {
        const errorMsg = formatError(err);
        return {
          canStart: false,
          reason: errorMsg,
          storageInfo: null,
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
