/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { afterEach, describe, expect, it, vi } from 'vitest';
import { createEventManager } from '../events';

const { listenMock } = vi.hoisted(() => ({
  listenMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: listenMock,
}));

describe('createEventManager', () => {
  afterEach(() => {
    listenMock.mockReset();
  });

  it('cleans up all successful registrations even when one unlistener throws', async () => {
    const firstUnlisten = vi.fn(() => {
      throw new Error('cleanup failed');
    });
    const secondUnlisten = vi.fn();

    listenMock
      .mockResolvedValueOnce(firstUnlisten)
      .mockResolvedValueOnce(secondUnlisten);

    const eventManager = createEventManager();

    await eventManager.registerAll([
      { name: 'server-started', handler: vi.fn() },
      { name: 'server-stopped', handler: vi.fn() },
    ]);

    eventManager.cleanup();

    expect(firstUnlisten).toHaveBeenCalledTimes(1);
    expect(secondUnlisten).toHaveBeenCalledTimes(1);
  });
});
