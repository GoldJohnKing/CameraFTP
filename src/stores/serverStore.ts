import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { ServerInfo, ServerStatus } from '../types';
import { formatError } from '../utils/error';
import { createEventManager, type EventRegistration } from '../utils/events';

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
  initializeListeners: () => Promise<() => void>;
}

const defaultStats: ServerStatus = {
  is_running: false,
  connected_clients: 0,
  files_received: 0,
  bytes_received: 0,
  last_file: null,
};

// 定义所有事件的注册配置
const createEventRegistrations = (get: () => ServerState, set: (fn: (state: ServerState) => ServerState) => void): EventRegistration<unknown>[] => [
  // 服务器启动事件
  {
    name: 'server-started',
    handler: (event) => {
      console.log('Server started event received:', event.payload);
      const { ip, port } = event.payload as { ip: string; port: number };
      set((state) => ({
        ...state,
        isRunning: true,
        serverInfo: {
          is_running: true,
          ip,
          port,
          url: `ftp://${ip}:${port}`,
          username: 'anonymous',
          password_info: '(任意密码)',
        },
        stats: { ...state.stats, is_running: true }
      }));
    },
  },
  // 服务器停止事件
  {
    name: 'server-stopped',
    handler: () => {
      set((state) => ({
        ...state,
        isRunning: false,
        serverInfo: null,
        stats: defaultStats
      }));
    },
  },
  // 统计更新事件
  {
    name: 'stats-update',
    handler: (event) => {
      console.log('Stats update received:', event.payload);
      set((state) => ({ ...state, stats: event.payload as ServerStatus }));
    },
  },
  // 文件上传事件
  {
    name: 'file-uploaded',
    handler: (event) => {
      const payload = event.payload as { path: string; size: number };
      console.log('File uploaded event received:', payload);
      
      // Android平台：触发媒体扫描让照片出现在相册中
      if (window.FileUploadAndroid?.onFileUploaded) {
        try {
          window.FileUploadAndroid.onFileUploaded(payload.path, payload.size);
          console.log('Media scan triggered for:', payload.path);
        } catch (err) {
          console.error('Failed to trigger media scan:', err);
        }
      }
    },
  },
  // 托盘菜单启动服务器
  {
    name: 'tray-start-server',
    handler: async () => {
      try {
        await get().startServer();
      } catch (err) {
        console.error('Failed to start server from tray:', err);
      }
    },
  },
  // 托盘菜单停止服务器
  {
    name: 'tray-stop-server',
    handler: async () => {
      try {
        await get().stopServer();
      } catch (err) {
        console.error('Failed to stop server from tray:', err);
      }
    },
  },
  // 窗口关闭请求
  {
    name: 'window-close-requested',
    handler: () => {
      window.dispatchEvent(new CustomEvent('app-quit-requested'));
    },
  },
  // Android 设置页面请求
  {
    name: 'android-open-manage-storage-settings',
    handler: () => {
      if (window.SAFPickerAndroid?.openAllFilesAccessSettings) {
        try {
          window.SAFPickerAndroid.openAllFilesAccessSettings();
        } catch (err) {
          console.error('Failed to open settings:', err);
        }
      } else {
        console.warn('SAFPickerAndroid.openAllFilesAccessSettings not available');
      }
    },
  },
];

// 同步服务器初始状态
const syncInitialState = async (set: (fn: (state: ServerState) => ServerState) => void): Promise<void> => {
  try {
    const info = await invoke<ServerInfo | null>('get_server_info');
    if (info?.is_running) {
      const status = await invoke<ServerStatus | null>('get_server_status');
      set((state) => ({
        ...state,
        isRunning: true,
        serverInfo: info,
        stats: status || { ...defaultStats, is_running: true },
      }));
    }
  } catch (err) {
    console.error('Failed to sync initial server state:', err);
  }
};

export const useServerStore = create<ServerState>((set, get) => ({
  isRunning: false,
  serverInfo: null,
  stats: defaultStats,
  isLoading: false,
  error: null,

  startServer: async () => {
    set((state) => ({ ...state, isLoading: true, error: null }));
    try {
      const info = await invoke<ServerInfo>('start_server');
      set((state) => ({
        ...state,
        isRunning: true,
        serverInfo: info,
        stats: { ...state.stats, is_running: true }
      }));
    } catch (err: unknown) {
      const errorMessage = formatError(err);
      set((state) => ({ ...state, error: errorMessage }));
      throw err;
    } finally {
      set((state) => ({ ...state, isLoading: false }));
    }
  },

  stopServer: async () => {
    set((state) => ({ ...state, isLoading: true, error: null }));
    try {
      await invoke('stop_server');
      set((state) => ({
        ...state,
        isRunning: false,
        serverInfo: null,
        stats: defaultStats
      }));
    } catch (err: unknown) {
      const errorMessage = formatError(err);
      set((state) => ({ ...state, error: errorMessage }));
      throw err;
    } finally {
      set((state) => ({ ...state, isLoading: false }));
    }
  },

  updateStats: (stats: ServerStatus) => {
    set((state) => ({ ...state, stats }));
  },

  setError: (error: string | null) => set((state) => ({ ...state, error })),
  clearError: () => set((state) => ({ ...state, error: null })),

  initializeListeners: async () => {
    const eventManager = createEventManager();
    
    // 使用 EventManager 批量注册所有事件
    await eventManager.registerAll(createEventRegistrations(get, set));
    
    // 同步初始状态
    await syncInitialState(set);

    // 返回清理函数
    return () => {
      eventManager.cleanup();
    };
  },
}));
