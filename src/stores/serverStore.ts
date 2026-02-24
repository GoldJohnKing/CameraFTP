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

    // === 核心服务器事件 ===

    // 监听服务器启动事件（当从托盘菜单启动时，更新前端状态）
    const unlistenStarted = await listen<{ ip: string; port: number }>('server-started', (event) => {
      console.log('Server started event received:', event.payload);
      const { ip, port } = event.payload;
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
      console.log('Stats update received:', event.payload);
      set({ stats: event.payload });
    });
    listeners.push(unlistenStats);

    // 监听文件上传事件（用于Android媒体扫描）
    const unlistenFileUploaded = await listen<{ path: string; size: number }>('file-uploaded', (event) => {
      console.log('File uploaded event received:', event.payload);
      
      // Android平台：触发媒体扫描让照片出现在相册中
      // @ts-ignore - FileUploadAndroid是Android注入的JS Bridge
      if (window.FileUploadAndroid && typeof window.FileUploadAndroid.onFileUploaded === 'function') {
        try {
          // @ts-ignore
          window.FileUploadAndroid.onFileUploaded(event.payload.path, event.payload.size);
          console.log('Media scan triggered for:', event.payload.path);
        } catch (err) {
          console.error('Failed to trigger media scan:', err);
        }
      }
    });
    listeners.push(unlistenFileUploaded);

    // === UI事件（托盘、窗口等）===

    // 托盘菜单启动服务器
    const unlistenTrayStart = await listen('tray-start-server', async () => {
      try {
        await get().startServer();
      } catch (err) {
        console.error('Failed to start server from tray:', err);
      }
    });
    listeners.push(unlistenTrayStart);

    // 托盘菜单停止服务器
    const unlistenTrayStop = await listen('tray-stop-server', async () => {
      try {
        await get().stopServer();
      } catch (err) {
        console.error('Failed to stop server from tray:', err);
      }
    });
    listeners.push(unlistenTrayStop);

    // 窗口关闭请求（显示退出对话框）
    const unlistenCloseRequested = await listen('window-close-requested', () => {
      // 通过自定义事件通知 App.tsx 显示退出对话框
      window.dispatchEvent(new CustomEvent('app-quit-requested'));
    });
    listeners.push(unlistenCloseRequested);

    // Android 设置页面请求
    const unlistenOpenSettings = await listen('android-open-manage-storage-settings', () => {
      // @ts-ignore - SAFPickerAndroid是Android注入的JS Bridge
      if (window.SAFPickerAndroid?.openAllFilesAccessSettings) {
        try {
          // @ts-ignore
          window.SAFPickerAndroid.openAllFilesAccessSettings();
        } catch (err) {
          console.error('Failed to open settings:', err);
        }
      } else {
        console.warn('SAFPickerAndroid.openAllFilesAccessSettings not available');
      }
    });
    listeners.push(unlistenOpenSettings);

    // 同步当前服务器状态（处理自启动场景：事件可能在监听器注册前已发出）
    try {
      const info = await invoke<ServerInfo | null>('get_server_info');
      if (info && info.is_running) {
        // 服务器已在运行，更新前端状态
        const status = await invoke<ServerStatus | null>('get_server_status');
        set({
          isRunning: true,
          serverInfo: info,
          stats: status || { ...defaultStats, is_running: true },
        });
      }
    } catch (err) {
      console.error('Failed to sync initial server state:', err);
    }

    // 返回清理函数
    return async () => {
      for (const unlisten of listeners) {
        unlisten();
      }
    };
  },
}));
