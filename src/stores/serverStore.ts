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
  setServerRunning: (serverInfo: ServerInfo, options?: { stats?: ServerStateSnapshot; immediate?: boolean }) => void;
  setServerStopped: (options?: { immediate?: boolean }) => void;
  setServerStats: (stats: ServerStateSnapshot) => void;
}

const defaultStats: ServerStateSnapshot = {
  isRunning: false,
  connectedClients: 0,
  filesReceived: 0,
  bytesReceived: 0,
  lastFile: null,
};

function createRunningStats(stats?: ServerStateSnapshot): ServerStateSnapshot {
  return {
    ...defaultStats,
    ...stats,
    isRunning: true,
    connectedClients: stats?.connectedClients ?? 0,
    filesReceived: stats?.filesReceived ?? 0,
    bytesReceived: stats?.bytesReceived ?? 0,
    lastFile: stats?.lastFile ?? null,
  };
}

function createStoppedStats(): ServerStateSnapshot {
  return { ...defaultStats };
}

const doStartServer = async (set: (fn: (state: ServerState) => ServerState) => void, get: () => ServerState): Promise<void> => {
  await executeAsync({
    operation: () => invoke<ServerInfo>('start_server'),
    onSuccess: (info) => {
      const currentState = get();
      get().setServerRunning(info, {
        stats: currentState.isRunning ? currentState.stats : undefined,
        immediate: true,
      });
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
      onSuccess: () => {
        get().setServerStopped();
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

  setServerRunning: (serverInfo, options) => {
    const stats = createRunningStats(options?.stats);
    set((state) => ({
      ...state,
      isRunning: true,
      serverInfo,
      stats,
    }));
    syncAndroidServerState(true, stats, stats.connectedClients, options?.immediate ?? false);
  },

  setServerStopped: (options) => {
    const stats = createStoppedStats();
    set((state) => ({
      ...state,
      isRunning: false,
      serverInfo: null,
      stats,
    }));
    syncAndroidServerState(false, null, 0, options?.immediate ?? false);
  },

  setServerStats: (stats) => {
    const nextStats = stats.isRunning ? createRunningStats(stats) : createStoppedStats();
    set((state) => ({ ...state, stats: nextStats, isRunning: nextStats.isRunning }));
    syncAndroidServerState(
      nextStats.isRunning,
      nextStats.isRunning ? nextStats : null,
      nextStats.connectedClients || 0,
      false,
    );
  },
}));
