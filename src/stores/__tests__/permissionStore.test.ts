/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { usePermissionStore } from '../permissionStore';
import { GALLERY_REFRESH_REQUESTED_EVENT } from '../../utils/gallery-refresh';

const { checkAllMock, invokeMock, permissionBridgeAvailableMock } = vi.hoisted(() => ({
  checkAllMock: vi.fn(),
  invokeMock: vi.fn(),
  permissionBridgeAvailableMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

vi.mock('../../types', () => ({
  permissionBridge: {
    isAvailable: permissionBridgeAvailableMock,
    checkAll: checkAllMock,
    requestStorage: vi.fn(),
    requestNotification: vi.fn(),
    requestBatteryOptimization: vi.fn(),
  },
}));

describe('permissionStore', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    usePermissionStore.setState({
      permissions: {
        storage: false,
        notification: false,
        batteryOptimization: false,
      },
      isLoading: false,
      error: null,
      isPolling: false,
      allGranted: false,
      isInitialized: false,
      storageInfo: null,
      needsPermission: false,
      pollingIntervalId: null,
    });
    checkAllMock.mockReset();
    invokeMock.mockReset();
    permissionBridgeAvailableMock.mockReset();
    permissionBridgeAvailableMock.mockReturnValue(true);
  });

  afterEach(() => {
    usePermissionStore.getState().stopPolling();
    vi.clearAllTimers();
    vi.useRealTimers();
  });

  it('refreshes the gallery when storage permission becomes granted', async () => {
    const refreshHandler = vi.fn();
    window.addEventListener(GALLERY_REFRESH_REQUESTED_EVENT, refreshHandler);

    checkAllMock
      .mockResolvedValueOnce({
        storage: false,
        notification: false,
        batteryOptimization: false,
      })
      .mockResolvedValueOnce({
        storage: true,
        notification: false,
        batteryOptimization: false,
      });

    usePermissionStore.getState().startPolling('storage');
    await Promise.resolve();

    await vi.advanceTimersByTimeAsync(300);

    expect(refreshHandler).toHaveBeenCalledTimes(1);
    expect(usePermissionStore.getState().isPolling).toBe(false);

    window.removeEventListener(GALLERY_REFRESH_REQUESTED_EVENT, refreshHandler);
  });

  it('maps checkPermissions result to permissions and allGranted', async () => {
    checkAllMock.mockResolvedValue({
      storage: true,
      notification: true,
      batteryOptimization: true,
    });

    const result = await usePermissionStore.getState().checkPermissions();

    expect(result).toEqual({
      storage: true,
      notification: true,
      batteryOptimization: true,
    });
    expect(usePermissionStore.getState().permissions).toEqual({
      storage: true,
      notification: true,
      batteryOptimization: true,
    });
    expect(usePermissionStore.getState().allGranted).toBe(true);
  });

  it('normalizes permissions to all granted when bridge is unavailable', async () => {
    permissionBridgeAvailableMock.mockReturnValue(false);

    const result = await usePermissionStore.getState().checkPermissions();

    expect(result).toEqual({
      storage: true,
      notification: true,
      batteryOptimization: true,
    });
    expect(usePermissionStore.getState().permissions).toEqual(result);
    expect(usePermissionStore.getState().allGranted).toBe(true);
  });

  it('maps check_permission_status needsUserAction to needsPermission', async () => {
    invokeMock.mockResolvedValue({
      hasAllFilesAccess: false,
      needsUserAction: true,
    });

    const result = await usePermissionStore.getState().checkPermissionStatus();

    expect(invokeMock).toHaveBeenCalledWith('check_permission_status');
    expect(result).toEqual({
      hasAllFilesAccess: false,
      needsUserAction: true,
    });
    expect(usePermissionStore.getState().needsPermission).toBe(true);
  });

  it('maps prerequisites response into storageInfo state', async () => {
    const storageInfo = {
      displayName: 'CameraFTP',
      path: '/storage/emulated/0/DCIM/CameraFTP',
      exists: true,
      writable: true,
      hasAllFilesAccess: true,
    };

    invokeMock.mockResolvedValue({
      canStart: true,
      reason: null,
      storageInfo,
    });

    const result = await usePermissionStore.getState().checkPrerequisites();

    expect(invokeMock).toHaveBeenCalledWith('check_server_start_prerequisites');
    expect(result).toEqual({
      canStart: true,
      reason: null,
      storageInfo,
    });
    expect(usePermissionStore.getState().storageInfo).toEqual(storageInfo);
  });

  it('normalizes prerequisite invoke failures into non-startable result', async () => {
    invokeMock.mockRejectedValue(new Error('bridge disconnected'));

    const result = await usePermissionStore.getState().checkPrerequisites();

    expect(result.canStart).toBe(false);
    expect(result.storageInfo).toBeNull();
    expect(result.reason).toContain('bridge disconnected');
  });
});
