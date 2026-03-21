/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useServerStore } from '../serverStore';

const {
  invokeMock,
  checkAndroidPermissionsMock,
  syncAndroidServerStateMock,
} = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  checkAndroidPermissionsMock: vi.fn(),
  syncAndroidServerStateMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

vi.mock('../../types', async () => {
  const actual = await vi.importActual<typeof import('../../types')>('../../types');
  return {
    ...actual,
    checkAndroidPermissions: checkAndroidPermissionsMock,
  };
});

vi.mock('../../services/android-server-state-sync', () => ({
  syncAndroidServerState: syncAndroidServerStateMock,
}));

describe('serverStore characterization', () => {
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

    invokeMock.mockReset();
    checkAndroidPermissionsMock.mockReset();
    syncAndroidServerStateMock.mockReset();

    invokeMock.mockImplementation(async (command: string) => {
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
      return null;
    });

    checkAndroidPermissionsMock.mockResolvedValue(null);
  });

  it('starts server when permissions are available', async () => {
    const started = await useServerStore.getState().startServer();

    expect(started).toBe(true);
    expect(invokeMock).toHaveBeenCalledWith('start_server');
    expect(useServerStore.getState().isRunning).toBe(true);
    expect(useServerStore.getState().serverInfo?.url).toBe('ftp://127.0.0.1:2221');
    expect(syncAndroidServerStateMock).toHaveBeenCalledWith(
      true,
      expect.objectContaining({ isRunning: true }),
      0,
      true,
    );
  });

  it('shows permission dialog when startServer prerequisites fail', async () => {
    checkAndroidPermissionsMock.mockResolvedValue({
      storage: false,
      notification: true,
      batteryOptimization: true,
    });

    const started = await useServerStore.getState().startServer();

    expect(started).toBe(false);
    expect(useServerStore.getState().showPermissionDialog).toBe(true);
    expect(useServerStore.getState().pendingServerStart).toBe(true);
    expect(invokeMock).not.toHaveBeenCalledWith('start_server');
  });

  it('stops server and resets runtime state', async () => {
    useServerStore.setState((state) => ({
      ...state,
      isRunning: true,
      serverInfo: {
        isRunning: true,
        ip: '127.0.0.1',
        port: 2221,
        url: 'ftp://127.0.0.1:2221',
        username: 'anonymous',
        passwordInfo: '(任意密码)',
      },
    }));

    await useServerStore.getState().stopServer();

    expect(invokeMock).toHaveBeenCalledWith('stop_server');
    expect(useServerStore.getState().isRunning).toBe(false);
    expect(useServerStore.getState().serverInfo).toBeNull();
    expect(syncAndroidServerStateMock).toHaveBeenCalledWith(false, null, 0);
  });
});
