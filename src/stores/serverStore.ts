import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { ServerInfo, ServerStatus } from '../types';
import { formatError } from '../utils/error';
import { createEventManager, type EventRegistration } from '../utils/events';
import { isPermissionAndroidAvailable } from '../types/global';
import type { PermissionCheckResult } from '../types/global';

interface ServerState {
  // 状态
  isRunning: boolean;
  serverInfo: ServerInfo | null;
  stats: ServerStatus;
  isLoading: boolean;
  error: string | null;
  showPermissionDialog: boolean;
  pendingServerStart: boolean;
  
  // 操作
  startServer: () => Promise<boolean>;
  stopServer: () => Promise<void>;
  updateStats: (stats: ServerStatus) => void;
  setError: (error: string | null) => void;
  clearError: () => void;
  openPermissionDialog: () => void;
  closePermissionDialog: () => void;
  continueAfterPermissionsGranted: () => Promise<void>;
  
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

// Update Android foreground service with current server state
const updateAndroidServiceState = (isRunning: boolean, stats: ServerStatus | null, connectedClients: number, immediate = false) => {
  console.log('[Android] updateAndroidServiceState called:', { isRunning, connectedClients, stats, immediate });
  console.log('[Android] window.ServerStateAndroid available:', !!window.ServerStateAndroid);
  
  // 立即模式：更多重试次数但更短间隔（用于服务器启动时）
  const MAX_RETRIES = immediate ? 30 : 5;
  const RETRY_DELAY_MS = immediate ? 50 : 200;
  
  const tryUpdate = (retriesLeft: number) => {
    if (window.ServerStateAndroid) {
      try {
        const statsJson = stats ? JSON.stringify({
          files_transferred: stats.files_received || 0,
          bytes_transferred: stats.bytes_received || 0,
        }) : null;
        
        console.log('[Android] Calling ServerStateAndroid.onServerStateChanged with:', { isRunning, statsJson, connectedClients });
        window.ServerStateAndroid.onServerStateChanged(isRunning, statsJson, connectedClients);
        console.log('[Android] ServerStateAndroid.onServerStateChanged call completed');
      } catch (e) {
        console.error('[Android] Failed to update service state:', e);
      }
    } else if (retriesLeft > 0) {
      console.warn(`[Android] ServerStateAndroid bridge not available, retrying in ${RETRY_DELAY_MS}ms (${MAX_RETRIES - retriesLeft + 1}/${MAX_RETRIES})`);
      setTimeout(() => tryUpdate(retriesLeft - 1), RETRY_DELAY_MS);
    } else {
      console.error('[Android] ServerStateAndroid bridge not available after retries - cannot update notification');
    }
  };
  
  tryUpdate(MAX_RETRIES);
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
      // Update Android foreground service
      updateAndroidServiceState(true, get().stats, 0);
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
      // Update Android foreground service
      updateAndroidServiceState(false, null, 0);
    },
  },
  // 统计更新事件
  {
    name: 'stats-update',
    handler: (event) => {
      console.log('Stats update received:', event.payload);
      const stats = event.payload as ServerStatus;
      set((state) => ({ ...state, stats }));
      // Update Android foreground service notification
      updateAndroidServiceState(true, stats, stats.connected_clients || 0);
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

// 实际启动服务器的逻辑（跳过权限检查）
const doStartServer = async (set: (fn: (state: ServerState) => ServerState) => void, get: () => ServerState): Promise<void> => {
  set((state) => ({ ...state, isLoading: true, error: null }));
  try {
    const info = await invoke<ServerInfo>('start_server');
    
    // 立即更新 Android 通知（在设置状态之前，确保最低延迟）
    // 使用局部变量而不是 get()，避免状态未更新问题
    const initialStats = { ...get().stats, is_running: true };
    // immediate=true: 使用更短间隔和更多重试，确保快速显示
    updateAndroidServiceState(true, initialStats, 0, true);
    
    set((state) => ({
      ...state,
      isRunning: true,
      serverInfo: info,
      stats: initialStats
    }));
  } catch (err: unknown) {
    const errorMessage = formatError(err);
    set((state) => ({ ...state, error: errorMessage }));
    throw err;
  } finally {
    set((state) => ({ ...state, isLoading: false }));
  }
};

// 检查 Android 权限
const checkAndroidPermissions = async (): Promise<PermissionCheckResult | null> => {
  if (!isPermissionAndroidAvailable()) {
    return null;
  }
  
  try {
    const result = await window.PermissionAndroid!.checkAllPermissions();
    return JSON.parse(result) as PermissionCheckResult;
  } catch (e) {
    console.error('Failed to check permissions:', e);
    return null;
  }
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
    set((state) => ({ ...state, isLoading: true, error: null }));
    try {
      await invoke('stop_server');
      set((state) => ({
        ...state,
        isRunning: false,
        serverInfo: null,
        stats: defaultStats
      }));
      // Update Android foreground service
      updateAndroidServiceState(false, null, 0);
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

  openPermissionDialog: () => set({ showPermissionDialog: true }),
  closePermissionDialog: () => set({ showPermissionDialog: false, pendingServerStart: false }),

  continueAfterPermissionsGranted: async () => {
    set({ showPermissionDialog: false, pendingServerStart: false });
    // Now actually start the server
    await doStartServer(set, get);
  },

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
