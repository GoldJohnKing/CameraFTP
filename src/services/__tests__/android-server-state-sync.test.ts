/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const { isAvailableMock, updateStateMock } = vi.hoisted(() => ({
  isAvailableMock: vi.fn(),
  updateStateMock: vi.fn(),
}));

vi.mock('../../types/global', () => ({
  serverStateBridge: {
    isAvailable: isAvailableMock,
    updateState: updateStateMock,
  },
}));

import { syncAndroidServerState } from '../android-server-state-sync';

describe('android server state sync', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    isAvailableMock.mockReset();
    updateStateMock.mockReset();
  });

  afterEach(() => {
    vi.runOnlyPendingTimers();
    vi.useRealTimers();
  });

  it('drops stale retry payloads after a newer state is queued', () => {
    isAvailableMock.mockReturnValue(false);

    syncAndroidServerState(true, {
      isRunning: true,
      connectedClients: 2,
      filesReceived: 7,
      bytesReceived: 1024,
      lastFile: '/older.jpg',
    }, 2);

    syncAndroidServerState(false, null, 0);

    isAvailableMock.mockReturnValue(true);
    vi.advanceTimersByTime(250);

    expect(updateStateMock).toHaveBeenCalledTimes(1);
    expect(updateStateMock).toHaveBeenCalledWith(
      false,
      null,
      0,
    );
  });

  it('preserves null stopped-state payloads when forwarding to the bridge', () => {
    isAvailableMock.mockReturnValue(true);

    syncAndroidServerState(false, null, 0);

    expect(updateStateMock).toHaveBeenCalledTimes(1);
    expect(updateStateMock).toHaveBeenCalledWith(false, null, 0);
  });

  it('keeps retrying the latest payload until the bridge becomes available', () => {
    isAvailableMock.mockReturnValue(false);

    syncAndroidServerState(false, null, 0, true);

    vi.advanceTimersByTime(2000);
    expect(updateStateMock).not.toHaveBeenCalled();

    isAvailableMock.mockReturnValue(true);
    vi.advanceTimersByTime(1000);

    expect(updateStateMock).toHaveBeenCalledTimes(1);
    expect(updateStateMock).toHaveBeenCalledWith(false, null, 0);
  });

  it('does not let a stale retry callback clear the newest retry timeout', () => {
    isAvailableMock.mockReturnValue(false);

    syncAndroidServerState(true, {
      isRunning: true,
      connectedClients: 1,
      filesReceived: 3,
      bytesReceived: 512,
      lastFile: '/older.jpg',
    }, 1);

    vi.advanceTimersByTime(1200);

    syncAndroidServerState(false, null, 0);

    vi.advanceTimersByTime(1000);
    expect(updateStateMock).not.toHaveBeenCalled();

    isAvailableMock.mockReturnValue(true);
    vi.advanceTimersByTime(1000);

    expect(updateStateMock).toHaveBeenCalledTimes(1);
    expect(updateStateMock).toHaveBeenCalledWith(false, null, 0);
  });

  it('does not schedule a redundant retry after the first sync succeeds', () => {
    isAvailableMock.mockReturnValue(true);
    updateStateMock.mockReturnValue(true);

    syncAndroidServerState(true, {
      isRunning: true,
      connectedClients: 3,
      filesReceived: 8,
      bytesReceived: 2048,
      lastFile: '/latest.jpg',
    }, 3);

    expect(updateStateMock).toHaveBeenCalledTimes(1);

    vi.advanceTimersByTime(3000);

    expect(updateStateMock).toHaveBeenCalledTimes(1);
  });
});
