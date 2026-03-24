/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { ServerInfo, ServerStateSnapshot } from '../types';
import { executeAsync } from '../utils/store';
import { checkAndroidPermissions } from '../types';
import { syncAndroidServerState } from '../services/android-server-state-sync';

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
}

const defaultStats: ServerStateSnapshot = {
  isRunning: false,
  connectedClients: 0,
  filesReceived: 0,
  bytesReceived: 0,
  lastFile: null,
};

const doStartServer = async (set: (fn: (state: ServerState) => ServerState) => void, get: () => ServerState): Promise<void> => {
  await executeAsync({
    operation: () => invoke<ServerInfo>('start_server'),
    onSuccess: (info, set) => {
      const initialStats = { ...get().stats, isRunning: true };
      syncAndroidServerState(true, initialStats, 0, true);
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
        syncAndroidServerState(false, null, 0);
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
}));
