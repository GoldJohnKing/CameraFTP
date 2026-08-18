/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useServerStore } from '../serverStore';
import { usePermissionStore } from '../permissionStore';

const {
  invokeMock,
  checkAndroidPermissionsMock,
} = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  checkAndroidPermissionsMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

vi.mock('../../services/permission-bridge', () => ({
  permissionBridge: {
    checkAll: checkAndroidPermissionsMock,
  },
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
    });
    usePermissionStore.setState({
      permissions: {
        storage: false,
        notification: false,
        batteryOptimization: false,
      },
      allGranted: false,
      hasCompletedFirstPermissionCheck: false,
    });

    invokeMock.mockReset();
    checkAndroidPermissionsMock.mockReset();
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
    expect(useServerStore.getState().stats).toEqual({
      isRunning: true,
      connectedClients: 0,
      filesReceived: 0,
      bytesReceived: 0,
      lastFile: null,
    });
  });

  it('preserves existing counters when startServer succeeds while already running', async () => {
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
      stats: {
        isRunning: true,
        connectedClients: 4,
        filesReceived: 11,
        bytesReceived: 8192,
        lastFile: '/keep.jpg',
      },
    }));

    const started = await useServerStore.getState().startServer();

    expect(started).toBe(true);
    expect(useServerStore.getState().stats).toEqual({
      isRunning: true,
      connectedClients: 4,
      filesReceived: 11,
      bytesReceived: 8192,
      lastFile: '/keep.jpg',
    });
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
    expect(invokeMock).not.toHaveBeenCalledWith('start_server');
  });

  it('syncs checked permissions into the permission store on startServer', async () => {
    checkAndroidPermissionsMock.mockResolvedValue({
      storage: true,
      notification: true,
      batteryOptimization: true,
    });

    const started = await useServerStore.getState().startServer();

    expect(started).toBe(true);
    expect(usePermissionStore.getState().permissions).toEqual({
      storage: true,
      notification: true,
      batteryOptimization: true,
    });
    expect(usePermissionStore.getState().allGranted).toBe(true);
    expect(invokeMock).toHaveBeenCalledWith('start_server');
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
  });

});
