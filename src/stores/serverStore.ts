import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { ServerInfo, ServerStatus } from '../types';
import { serverStateBridge, storageSettingsBridge } from '../types/global';
import { createEventManager, type EventRegistration } from '../utils/events';
import { retryAction, executeAsync } from '../utils/store';
import { checkAndroidPermissions } from '../types';

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
  closePermissionDialog: () => void;
  continueAfterPermissionsGranted: () => Promise<void>;
  
  // 初始化
  initializeListeners: () => Promise<() => void>;
}

const defaultStats: ServerStatus = {
  isRunning: false,
  connectedClients: 0,
  filesReceived: 0,
  bytesReceived: 0,
  lastFile: null,
};

// Update Android foreground service with current server state
const updateAndroidServiceState = (isRunning: boolean, stats: ServerStatus | null, connectedClients: number, immediate = false) => {
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

// Define all event registrations
const createEventRegistrations = (get: () => ServerState, set: (fn: (state: ServerState) => ServerState) => void): EventRegistration<unknown>[] => [
  {
    name: 'server-started',
    handler: (event) => {
      const { ip, port } = event.payload as { ip: string; port: number };
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
    handler: (event) => {
      const stats = event.payload as ServerStatus;
      set((state) => ({ ...state, stats }));
      updateAndroidServiceState(true, stats, stats.connectedClients || 0);
    },
  },
  {
    name: 'file-uploaded',
    handler: (event) => {
      const payload = event.payload as { path: string; size: number };
      if (window.FileUploadAndroid?.onFileUploaded) {
        try {
          window.FileUploadAndroid.onFileUploaded(payload.path, payload.size);
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
      } catch {
        // Silently ignore tray start server errors
      }
    },
  },
  {
    name: 'tray-stop-server',
    handler: async () => {
      try {
        await get().stopServer();
      } catch {
        // Silently ignore tray stop server errors
      }
    },
  },
  {
    name: 'window-close-requested',
    handler: async () => {
      // 先显示并置顶主窗口，确保对话框可见
      try {
        await invoke('show_main_window');
      } catch {
        // 忽略窗口显示错误
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

// Sync initial server state
const syncInitialState = async (set: (fn: (state: ServerState) => ServerState) => void): Promise<void> => {
  try {
    const info = await invoke<ServerInfo | null>('get_server_info');
    if (info?.isRunning) {
      const status = await invoke<ServerStatus | null>('get_server_status');
      set((state) => ({
        ...state,
        isRunning: true,
        serverInfo: info,
        stats: status || { ...defaultStats, isRunning: true },
      }));
    }
  } catch {
    // Silently ignore initial state sync errors
  }
};

// Start server logic (permission check skipped)
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
