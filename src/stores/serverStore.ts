import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { ServerInfo, ServerStatus } from '../types';

interface ServerState {
  // 状态
  isRunning: boolean;
  serverInfo: ServerInfo | null;
  stats: ServerStatus;
  isLoading: boolean;
  error: string | null;
  
  // 操作
  startServer: () => Promise<void>;
  stopServer: () => Promise<void>;
  updateStats: (stats: ServerStatus) => void;
  setError: (error: string | null) => void;
  clearError: () => void;
  
  // 初始化
  initializeListeners: () => Promise<() => Promise<void>>;
}

const defaultStats: ServerStatus = {
  is_running: false,
  connected_clients: 0,
  files_received: 0,
  bytes_received: 0,
  last_file: null,
};

export const useServerStore = create<ServerState>((set, get) => ({
  isRunning: false,
  serverInfo: null,
  stats: defaultStats,
  isLoading: false,
  error: null,

  startServer: async () => {
    set({ isLoading: true, error: null });
    try {
      const info = await invoke<ServerInfo>('start_server');
      set({
        isRunning: true,
        serverInfo: info,
        stats: { ...get().stats, is_running: true }
      });
    } catch (err: any) {
      const errorMessage = err?.message || String(err);
      set({ error: errorMessage });
      throw err;
    } finally {
      set({ isLoading: false });
    }
  },

  stopServer: async () => {
    set({ isLoading: true, error: null });
    try {
      await invoke('stop_server');
      set({
        isRunning: false,
        serverInfo: null,
        stats: defaultStats
      });
    } catch (err: any) {
      const errorMessage = err?.message || String(err);
      set({ error: errorMessage });
      throw err;
    } finally {
      set({ isLoading: false });
    }
  },

  updateStats: (stats: ServerStatus) => {
    set({ stats });
  },

  setError: (error: string | null) => set({ error }),
  clearError: () => set({ error: null }),

  initializeListeners: async () => {
    const listeners: UnlistenFn[] = [];

    // 监听服务器启动事件（当从托盘菜单启动时，更新前端状态）
    const unlistenStarted = await listen<[string, number]>('server-started', (event) => {
      console.log('Server started event received:', event.payload);
      const [ip, port] = event.payload;
      set({
        isRunning: true,
        serverInfo: {
          is_running: true,
          ip,
          port,
          url: `ftp://${ip}:${port}`,
          username: 'anonymous',
          password_info: '(任意密码)',
        },
        stats: { ...get().stats, is_running: true }
      });
    });
    listeners.push(unlistenStarted);

    // 监听服务器停止事件
    const unlistenStopped = await listen('server-stopped', () => {
      set({ 
        isRunning: false, 
        serverInfo: null,
        stats: defaultStats
      });
    });
    listeners.push(unlistenStopped);

    // 监听统计更新事件（用于替代轮询）
    const unlistenStats = await listen<ServerStatus>('stats-update', (event) => {
      set({ stats: event.payload });
    });
    listeners.push(unlistenStats);

    // 返回清理函数
    return async () => {
      for (const unlisten of listeners) {
        unlisten();
      }
    };
  },
}));
