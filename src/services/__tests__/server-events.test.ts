/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { initializeServerEvents } from '../server-events';
import { useServerStore } from '../../stores/serverStore';

const {
  invokeMock,
  listenMock,
  checkAndroidPermissionsMock,
  openStorageSettingsMock,
  scheduleMediaLibraryRefreshMock,
  shouldScheduleUploadRefreshMock,
} = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  listenMock: vi.fn(),
  checkAndroidPermissionsMock: vi.fn(),
  openStorageSettingsMock: vi.fn(),
  scheduleMediaLibraryRefreshMock: vi.fn(),
  shouldScheduleUploadRefreshMock: vi.fn(),
}));

const eventHandlers = new Map<string, (event: { payload: unknown }) => void | Promise<void>>();

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: listenMock,
}));

vi.mock('../../types', async () => {
  const actual = await vi.importActual<typeof import('../../types')>('../../types');
  return {
    ...actual,
    checkAndroidPermissions: checkAndroidPermissionsMock,
  };
});

vi.mock('../../types/global', async () => {
  const actual = await vi.importActual<typeof import('../../types/global')>('../../types/global');
  return {
    ...actual,
    storageSettingsBridge: {
      isAvailable: () => true,
      openAllFilesAccessSettings: openStorageSettingsMock,
    },
  };
});

vi.mock('../../utils/gallery-refresh', () => ({
  scheduleMediaLibraryRefresh: scheduleMediaLibraryRefreshMock,
}));

vi.mock('../../utils/server-stats-refresh', () => ({
  shouldScheduleUploadRefresh: shouldScheduleUploadRefreshMock,
}));

describe('server event lifecycle service', () => {
  beforeEach(() => {
    useServerStore.setState({
      isRunning: false,
      serverInfo: null,
      stats: {
        isRunning: false,
        connectedClients: 0,
        filesReceived: 0,
        bytesReceived: 0,
        lastFile: null,
      },
      isLoading: false,
      error: null,
      showPermissionDialog: false,
      pendingServerStart: false,
    });

    eventHandlers.clear();
    invokeMock.mockReset();
    listenMock.mockReset();
    checkAndroidPermissionsMock.mockReset();
    openStorageSettingsMock.mockReset();
    scheduleMediaLibraryRefreshMock.mockReset();
    shouldScheduleUploadRefreshMock.mockReset();
    shouldScheduleUploadRefreshMock.mockReturnValue(false);

    listenMock.mockImplementation(async (name: string, handler: (event: { payload: unknown }) => void) => {
      eventHandlers.set(name, handler);
      return vi.fn();
    });

    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'get_server_runtime_state') {
        return {
          serverInfo: null,
          stats: null,
        };
      }
      if (command === 'start_server') {
        return {
          isRunning: true,
          ip: '127.0.0.1',
          port: 2221,
          url: 'ftp://127.0.0.1:2221',
          username: 'anonymous',
          passwordInfo: '(任意密码)',
        };
      }
      if (command === 'stop_server') {
        return null;
      }
      if (command === 'show_main_window') {
        return null;
      }
      return null;
    });

    checkAndroidPermissionsMock.mockResolvedValue(null);
  });

  afterEach(() => {
    eventHandlers.clear();
  });

  it('registers listeners, syncs initial state, and applies start/stop updates', async () => {
    useServerStore.setState((state) => ({
      ...state,
      stats: {
        isRunning: false,
        connectedClients: 3,
        filesReceived: 9,
        bytesReceived: 4096,
        lastFile: '/stale.jpg',
      },
    }));

    const cleanup = await initializeServerEvents();

    expect(invokeMock).toHaveBeenCalledWith('get_server_runtime_state');
    expect(eventHandlers.has('server-started')).toBe(true);
    expect(eventHandlers.has('server-stopped')).toBe(true);
    expect(eventHandlers.has('stats-update')).toBe(true);
    expect(eventHandlers.has('tray-start-server')).toBe(true);

    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'get_server_runtime_state') {
        return {
          serverInfo: {
            isRunning: true,
            ip: '192.168.1.8',
            port: 2121,
            url: 'ftp://192.168.1.8:2121',
            username: 'anonymous',
            passwordInfo: '(任意密码)',
          },
          stats: {
            isRunning: true,
            connectedClients: 0,
            filesReceived: 0,
            bytesReceived: 0,
            lastFile: null,
          },
        };
      }
      return null;
    });

    await eventHandlers.get('server-started')?.({ payload: { ip: '192.168.1.8', port: 2121 } });
    expect(useServerStore.getState().isRunning).toBe(true);
    expect(useServerStore.getState().serverInfo?.url).toBe('ftp://192.168.1.8:2121');
    expect(useServerStore.getState().stats).toEqual({
      isRunning: true,
      connectedClients: 0,
      filesReceived: 0,
      bytesReceived: 0,
      lastFile: null,
    });
    await eventHandlers.get('server-stopped')?.({ payload: undefined });
    expect(useServerStore.getState().isRunning).toBe(false);
    expect(useServerStore.getState().serverInfo).toBeNull();
    expect(useServerStore.getState().stats).toEqual({
      isRunning: false,
      connectedClients: 0,
      filesReceived: 0,
      bytesReceived: 0,
      lastFile: null,
    });
    cleanup();
  });

  it('hydrates store state during initial sync when server is already running', async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'get_server_runtime_state') {
        return {
          serverInfo: {
            isRunning: true,
            ip: '192.168.1.99',
            port: 2121,
            url: 'ftp://192.168.1.99:2121',
            username: 'anonymous',
            passwordInfo: '(任意密码)',
          },
          stats: {
            isRunning: true,
            connectedClients: 2,
            filesReceived: 7,
            bytesReceived: 2048,
            lastFile: null,
          },
        };
      }
      return null;
    });

    await initializeServerEvents();

    expect(useServerStore.getState().isRunning).toBe(true);
    expect(useServerStore.getState().stats).toEqual({
      isRunning: true,
      connectedClients: 2,
      filesReceived: 7,
      bytesReceived: 2048,
      lastFile: null,
    });
  });

  it('refreshes full server info on server-started instead of hardcoding anonymous credentials', async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'get_server_runtime_state') {
        return {
          serverInfo: {
            isRunning: true,
            ip: '192.168.1.50',
            port: 2121,
            url: 'ftp://192.168.1.50:2121',
            username: 'camera',
            passwordInfo: '已设置密码',
          },
          stats: {
            isRunning: true,
            connectedClients: 1,
            filesReceived: 3,
            bytesReceived: 512,
            lastFile: '/latest.jpg',
          },
        };
      }
      return null;
    });

    await initializeServerEvents();
    invokeMock.mockClear();

    await eventHandlers.get('server-started')?.({ payload: { ip: '192.168.1.50', port: 2121 } });

    expect(invokeMock).toHaveBeenCalledWith('get_server_runtime_state');
    expect(useServerStore.getState().serverInfo).toEqual({
      isRunning: true,
      ip: '192.168.1.50',
      port: 2121,
      url: 'ftp://192.168.1.50:2121',
      username: 'camera',
      passwordInfo: '已设置密码',
    });
  });

  it('normalizes fallback server-started payloads to the IPv4-only contract', async () => {
    await initializeServerEvents();

    invokeMock.mockRejectedValueOnce(new Error('runtime sync failed'));

    await eventHandlers.get('server-started')?.({ payload: { ip: '::1', port: 2121 } });

    expect(useServerStore.getState().serverInfo).toEqual({
      isRunning: true,
      ip: '127.0.0.1',
      port: 2121,
      url: 'ftp://127.0.0.1:2121',
      username: 'anonymous',
      passwordInfo: '(任意密码)',
    });
  });

  it('reconciles stopped backend state during initial sync', async () => {
    useServerStore.setState({
      isRunning: true,
      serverInfo: {
        isRunning: true,
        ip: '192.168.1.10',
        port: 2121,
        url: 'ftp://192.168.1.10:2121',
        username: 'anonymous',
        passwordInfo: '(任意密码)',
      },
      stats: {
        isRunning: true,
        connectedClients: 2,
        filesReceived: 5,
        bytesReceived: 2048,
        lastFile: '/stale.jpg',
      },
    });

    await initializeServerEvents();

    expect(useServerStore.getState().isRunning).toBe(false);
    expect(useServerStore.getState().serverInfo).toBeNull();
    expect(useServerStore.getState().stats).toEqual({
      isRunning: false,
      connectedClients: 0,
      filesReceived: 0,
      bytesReceived: 0,
      lastFile: null,
    });
  });

  it('delegates lifecycle ownership to store setters without bridge work', async () => {
    await initializeServerEvents();

    const originalState = useServerStore.getState();
    const setServerRunning = vi.fn();
    const setServerStopped = vi.fn();
    const setServerStats = vi.fn();

    useServerStore.setState({
      setServerRunning,
      setServerStopped,
      setServerStats,
    });

    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'get_server_runtime_state') {
        return {
          serverInfo: {
            isRunning: true,
            ip: '192.168.1.8',
            port: 2121,
            url: 'ftp://192.168.1.8:2121',
            username: 'anonymous',
            passwordInfo: '(任意密码)',
          },
          stats: {
            isRunning: true,
            connectedClients: 0,
            filesReceived: 0,
            bytesReceived: 0,
            lastFile: null,
          },
        };
      }
      return null;
    });

    await eventHandlers.get('server-started')?.({ payload: { ip: '192.168.1.8', port: 2121 } });
    await eventHandlers.get('stats-update')?.({
      payload: {
        isRunning: true,
        connectedClients: 1,
        filesReceived: 1,
        bytesReceived: 100,
        lastFile: null,
      },
    });
    await eventHandlers.get('server-stopped')?.({ payload: undefined });

    expect(setServerRunning).toHaveBeenCalledTimes(1);
    expect(setServerStats).toHaveBeenCalledTimes(1);
    expect(setServerStopped).toHaveBeenCalledTimes(1);

    useServerStore.setState({
      setServerRunning: originalState.setServerRunning,
      setServerStopped: originalState.setServerStopped,
      setServerStats: originalState.setServerStats,
    });
  });

  it('handles tray start, stats refresh, and storage settings bridge events', async () => {
    await initializeServerEvents();

    await eventHandlers.get('tray-start-server')?.({ payload: undefined });
    expect(invokeMock).toHaveBeenCalledWith('start_server');

    shouldScheduleUploadRefreshMock.mockReturnValue(true);
    await eventHandlers.get('stats-update')?.({
      payload: {
        isRunning: true,
        connectedClients: 1,
        filesReceived: 1,
        bytesReceived: 100,
        lastFile: null,
      },
    });
    expect(useServerStore.getState().stats).toEqual({
      isRunning: true,
      connectedClients: 1,
      filesReceived: 1,
      bytesReceived: 100,
      lastFile: null,
    });
    await eventHandlers.get('android-open-manage-storage-settings')?.({ payload: undefined });
    expect(openStorageSettingsMock).toHaveBeenCalledTimes(1);
  });
});
