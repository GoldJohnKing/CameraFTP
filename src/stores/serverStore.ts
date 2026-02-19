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
    } catch (err) {
      set({ error: String(err) });
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
    } catch (err) {
      set({ error: String(err) });
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

    // 监听服务器启动事件
    const unlistenStarted = await listen<[string, number]>('server-started', () => {
      // 服务器已启动，可以在这里做额外处理
      console.log('Server started event received');
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
