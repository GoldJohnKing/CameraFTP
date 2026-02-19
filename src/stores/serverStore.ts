/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { Event } from '@tauri-apps/api/event';
import type { ServerInfo, ServerStateSnapshot } from '../types';
import { serverStateBridge, storageSettingsBridge } from '../types/global';
import { createEventManager, type EventRegistration } from '../utils/events';
import { retryAction, executeAsync } from '../utils/store';
import { checkAndroidPermissions } from '../types';

// TypeScript declarations for window extensions
declare global {
  interface Window {
    FileUploadAndroid?: {
      onFileUploaded: (path: string, size: number) => void;
    };
  }
}

// Event payload types
type ServerStartedPayload = { ip: string; port: number };
type FileUploadedPayload = { path: string; size: number };

interface ServerState {
  // 状态
  isRunning: boolean;
  serverInfo: ServerInfo | null;
  stats: ServerStateSnapshot;
  isLoading: boolean;
  error: string | null;
  showPermissionDialog: boolean;
  pendingServerStart: boolean;
  
  // 操作
  startServer: () => Promise<boolean>;
  stopServer: () => Promise<void>;
  closePermissionDialog: () => void;
  continueAfterPermissionsGranted: () => Promise<void>;
  
  // 初始化
  initializeListeners: () => Promise<() => void>;
}

const defaultStats: ServerStateSnapshot = {
  isRunning: false,
  connectedClients: 0,
  filesReceived: 0,
  bytesReceived: 0,
  lastFile: null,
};

const updateAndroidServiceState = (isRunning: boolean, stats: ServerStateSnapshot | null, connectedClients: number, immediate = false) => {
  retryAction(
    () => {
      if (!serverStateBridge.isAvailable()) return false;
      const statsJson = stats ? JSON.stringify({
        files_transferred: stats.filesReceived || 0,
        bytes_transferred: stats.bytesReceived || 0,
      }) : null;
      return serverStateBridge.updateState(isRunning, statsJson, connectedClients);
    },
    { maxRetries: immediate ? 30 : 5, delayMs: immediate ? 50 : 200 }
  );
};

const createEventRegistrations = (
  get: () => ServerState,
  set: (fn: (state: ServerState) => ServerState) => void
): EventRegistration<any>[] => [
  {
    name: 'server-started',
    handler: (event: Event<ServerStartedPayload>) => {
      const { ip, port } = event.payload;
      set((state) => ({
        ...state,
        isRunning: true,
        serverInfo: {
          isRunning: true,
          ip,
          port,
          url: `ftp://${ip}:${port}`,
          username: 'anonymous',
          passwordInfo: '(任意密码)',
        },
        stats: { ...state.stats, isRunning: true }
      }));
      updateAndroidServiceState(true, get().stats, 0);
    },
  },
  {
    name: 'server-stopped',
    handler: () => {
      set((state) => ({
        ...state,
        isRunning: false,
        serverInfo: null,
        stats: defaultStats
      }));
      updateAndroidServiceState(false, null, 0);
    },
  },
  {
    name: 'stats-update',
    handler: (event: Event<ServerStateSnapshot>) => {
      const stats = event.payload;
      set((state) => ({ ...state, stats }));
      updateAndroidServiceState(true, stats, stats.connectedClients || 0);
    },
  },
  {
    name: 'file-uploaded',
    handler: (event: Event<FileUploadedPayload>) => {
      const { path, size } = event.payload;
      if (window.FileUploadAndroid?.onFileUploaded) {
        try {
          window.FileUploadAndroid.onFileUploaded(path, size);
        } catch {
          // Silently ignore media scan errors
        }
      }
    },
  },
  {
    name: 'tray-start-server',
    handler: async () => {
      try {
        await get().startServer();
      } catch (err) {
        console.warn('[serverStore] Tray start server failed:', err);
      }
    },
  },
  {
    name: 'tray-stop-server',
    handler: async () => {
      try {
        await get().stopServer();
      } catch (err) {
        console.warn('[serverStore] Tray stop server failed:', err);
      }
    },
  },
  {
    name: 'window-close-requested',
    handler: async () => {
      try {
        await invoke('show_main_window');
      } catch {
        // Ignore window display errors
      }
      window.dispatchEvent(new CustomEvent('app-quit-requested'));
    },
  },
  {
    name: 'android-open-manage-storage-settings',
    handler: () => {
      storageSettingsBridge.openAllFilesAccessSettings();
    },
  },
];

const syncInitialState = async (set: (fn: (state: ServerState) => ServerState) => void): Promise<void> => {
  try {
    const info = await invoke<ServerInfo | null>('get_server_info');
    if (info?.isRunning) {
      const status = await invoke<ServerStateSnapshot | null>('get_server_status');
      set((state) => ({
        ...state,
        isRunning: true,
        serverInfo: info,
        stats: status || { ...defaultStats, isRunning: true },
      }));
    }
  } catch (err) {
    console.warn('[serverStore] Initial state sync failed:', err);
  }
};

const doStartServer = async (set: (fn: (state: ServerState) => ServerState) => void, get: () => ServerState): Promise<void> => {
  await executeAsync({
    operation: () => invoke<ServerInfo>('start_server'),
    onSuccess: (info, set) => {
      const initialStats = { ...get().stats, isRunning: true };
      updateAndroidServiceState(true, initialStats, 0, true);
      set((state) => ({
        ...state,
        isRunning: true,
        serverInfo: info,
        stats: initialStats
      }));
    },
    errorPrefix: 'Failed to start server',
    rethrow: true,
  }, set);
};

export const useServerStore = create<ServerState>((set, get) => ({
  isRunning: false,
  serverInfo: null,
  stats: defaultStats,
  isLoading: false,
  error: null,
  showPermissionDialog: false,
  pendingServerStart: false,

  startServer: async () => {
    // Check if we're on Android and need to check permissions
    const permissions = await checkAndroidPermissions();
    
    if (permissions !== null) {
      if (!permissions.storage || !permissions.notification || !permissions.batteryOptimization) {
        // Show permission dialog instead of starting server
        set({ showPermissionDialog: true, pendingServerStart: true });
        return false; // Return false to indicate server was NOT started
      }
    }
    
    // Permissions OK or not on Android, proceed to start
    await doStartServer(set, get);
    return true; // Return true to indicate server was successfully started
  },

  stopServer: async () => {
    await executeAsync({
      operation: () => invoke('stop_server'),
      onSuccess: (_, set) => {
        set((state) => ({
          ...state,
          isRunning: false,
          serverInfo: null,
          stats: defaultStats
        }));
        updateAndroidServiceState(false, null, 0);
      },
      errorPrefix: 'Failed to stop server',
      rethrow: true,
    }, set);
  },

  closePermissionDialog: () => set({ showPermissionDialog: false, pendingServerStart: false }),

  continueAfterPermissionsGranted: async () => {
    set({ showPermissionDialog: false, pendingServerStart: false });
    // Now actually start the server
    await doStartServer(set, get);
  },

  initializeListeners: async () => {
    const eventManager = createEventManager();

    await eventManager.registerAll(createEventRegistrations(get, set));

    await syncInitialState(set);

    return () => {
      eventManager.cleanup();
    };
  },
}));
