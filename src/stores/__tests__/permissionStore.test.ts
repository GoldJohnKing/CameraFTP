/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { usePermissionStore } from '../permissionStore';
import { GALLERY_REFRESH_REQUESTED_EVENT } from '../../utils/gallery-refresh';

const { checkAllMock } = vi.hoisted(() => ({
  checkAllMock: vi.fn(),
}));

vi.mock('../../types', () => ({
  permissionBridge: {
    isAvailable: () => true,
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
});
