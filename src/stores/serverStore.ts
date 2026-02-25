import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { ServerInfo, ServerStatus } from '../types';
import { formatError } from '../utils/error';
import { createEventManager, type EventRegistration } from '../utils/events';
import { checkAndroidPermissions } from '../types/global';

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
  is_running: false,
  connected_clients: 0,
  files_received: 0,
  bytes_received: 0,
  last_file: null,
};

// Update Android foreground service with current server state
const updateAndroidServiceState = (isRunning: boolean, stats: ServerStatus | null, connectedClients: number, immediate = false) => {
  const MAX_RETRIES = immediate ? 30 : 5;
  const RETRY_DELAY_MS = immediate ? 50 : 200;

  const tryUpdate = (retriesLeft: number) => {
    if (window.ServerStateAndroid) {
      try {
        const statsJson = stats ? JSON.stringify({
          files_transferred: stats.files_received || 0,
          bytes_transferred: stats.bytes_received || 0,
        }) : null;
        window.ServerStateAndroid.onServerStateChanged(isRunning, statsJson, connectedClients);
      } catch {
        // Silently ignore Android service state update errors
      }
    } else if (retriesLeft > 0) {
      setTimeout(() => tryUpdate(retriesLeft - 1), RETRY_DELAY_MS);
    }
  };

  tryUpdate(MAX_RETRIES);
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
          is_running: true,
          ip,
          port,
          url: `ftp://${ip}:${port}`,
          username: 'anonymous',
          password_info: '(任意密码)',
        },
        stats: { ...state.stats, is_running: true }
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
      updateAndroidServiceState(true, stats, stats.connected_clients || 0);
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
    handler: () => {
      window.dispatchEvent(new CustomEvent('app-quit-requested'));
    },
  },
  {
    name: 'android-open-manage-storage-settings',
    handler: () => {
      if (window.StorageSettingsAndroid?.openAllFilesAccessSettings) {
        try {
          window.StorageSettingsAndroid.openAllFilesAccessSettings();
        } catch {
          // Silently ignore settings open errors
        }
      }
    },
  },
];

// Sync initial server state
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
  } catch {
    // Silently ignore initial state sync errors
  }
};

// Start server logic (permission check skipped)
const doStartServer = async (set: (fn: (state: ServerState) => ServerState) => void, get: () => ServerState): Promise<void> => {
  set((state) => ({ ...state, isLoading: true, error: null }));
  try {
    const info = await invoke<ServerInfo>('start_server');

    const initialStats = { ...get().stats, is_running: true };
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
