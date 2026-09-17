/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { ServerInfo, ServerStateSnapshot } from '../types';
import { executeAsync } from '../utils/store';
import { permissionBridge } from '../services/permission-bridge';
import { usePermissionStore } from './permissionStore';

interface ServerState {
  isRunning: boolean;
  serverInfo: ServerInfo | null;
  stats: ServerStateSnapshot;
  isLoading: boolean;
  error: string | null;
  showPermissionDialog: boolean;

  startServer: () => Promise<boolean>;
  stopServer: () => Promise<void>;
  closePermissionDialog: () => void;
  continueAfterPermissionsGranted: () => Promise<void>;
  setServerRunning: (serverInfo: ServerInfo, options?: { stats?: ServerStateSnapshot }) => void;
  setServerStopped: () => void;
  setServerStats: (stats: ServerStateSnapshot) => void;
}

const defaultStats: ServerStateSnapshot = {
  isRunning: false,
  connectedClients: 0,
  filesReceived: 0,
  bytesReceived: 0,
  lastFile: null,
};

// 同步防重窗口：托盘事件直接调用 store（绕过 UI 本地态），而 isLoading 要
// 到 checkAll IPC 完成后才由 executeAsync 置位 —— 在此之前 UI 按钮 + 托盘
// 菜单可并发进入 startServer/continueAfterPermissionsGranted。用模块级同步
// flag 在入口第一步即封住整个 in-flight 窗口（含 checkAll / 权限弹窗早退 /
// doStartServer 成败所有路径，try/finally 保证复位），杜绝并发调用让第二个
// 调用者收到误导性的 ServerAlreadyRunning。startInFlight 是唯一权威防线：
// 它先行同步置位。已知残余：stopServer 的 isLoading 窗口内 start 不被前端
// 拦截——后端 start_server 幂等兜底，最坏一次误导性错误提示，自愈。
let startInFlight = false;

function createRunningStats(stats?: ServerStateSnapshot): ServerStateSnapshot {
  return {
    isRunning: true,
    connectedClients: stats?.connectedClients ?? 0,
    filesReceived: stats?.filesReceived ?? 0,
    bytesReceived: stats?.bytesReceived ?? 0,
    lastFile: stats?.lastFile ?? null,
  };
}

async function doStartServer(set: (fn: (state: ServerState) => ServerState) => void, get: () => ServerState): Promise<void> {
  await executeAsync({
    operation: () => invoke<ServerInfo>('start_server'),
    onSuccess: (info) => {
      const currentState = get();
      get().setServerRunning(info, {
        stats: currentState.isRunning ? currentState.stats : undefined,
      });
    },
    errorPrefix: 'Failed to start server',
    rethrow: true,
  }, set);
}

export const useServerStore = create<ServerState>((set, get) => ({
  isRunning: false,
  serverInfo: null,
  stats: defaultStats,
  isLoading: false,
  error: null,
  showPermissionDialog: false,

  startServer: async () => {
    if (startInFlight) return false;
    startInFlight = true;
    try {
      const permissions = await permissionBridge.checkAll();

      if (permissions !== null) {
        // Keep the permission store in sync on the "start without opening the
        // settings page" path; setPermissions also hosts the storage
        // false→true gallery-refresh transition hook.
        usePermissionStore.getState().setPermissions(permissions);
        if (!permissions.storage || !permissions.notification || !permissions.batteryOptimization) {
          set({ showPermissionDialog: true });
          return false;
        }
      }

      await doStartServer(set, get);
      return true;
    } finally {
      startInFlight = false;
    }
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

  closePermissionDialog: () => set({ showPermissionDialog: false }),

  continueAfterPermissionsGranted: async () => {
    if (startInFlight) return;
    startInFlight = true;
    try {
      set({ showPermissionDialog: false });
      await doStartServer(set, get);
    } finally {
      startInFlight = false;
    }
  },

  setServerRunning: (serverInfo, options) => {
    const stats = createRunningStats(options?.stats);
    set({
      isRunning: true,
      serverInfo,
      stats,
      // 服务已确认运行，权限对话框不应继续挂在运行中的服务上（另一入口
      // 启动成功时清除残留的对话框状态）。
      showPermissionDialog: false,
    });
  },

  setServerStopped: () => {
    set({
      isRunning: false,
      serverInfo: null,
      stats: defaultStats,
    });
  },

  setServerStats: (stats) => {
    const nextStats = stats.isRunning ? createRunningStats(stats) : defaultStats;
    set({ stats: nextStats, isRunning: nextStats.isRunning });
  },
}));
