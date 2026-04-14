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

  it('registers events and delivers events to handlers', async () => {
    const handler = vi.fn();
    listenMock.mockResolvedValueOnce(vi.fn());

    const eventManager = createEventManager();
    await eventManager.registerAll([
      { name: 'test-event', handler },
    ]);

    const registeredHandler = listenMock.mock.calls[0]?.[1];
    expect(registeredHandler).toBeDefined();

    const mockEvent = { event: 'test-event', payload: { data: 42 } };
    registeredHandler?.(mockEvent);

    expect(handler).toHaveBeenCalledTimes(1);
    expect(handler).toHaveBeenCalledWith(mockEvent);
  });

  it('handles empty registration array', async () => {
    const eventManager = createEventManager();
    await eventManager.registerAll([]);
    expect(listenMock).not.toHaveBeenCalled();
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

  it('cleanup is idempotent', async () => {
    const unlisten = vi.fn();
    listenMock.mockResolvedValueOnce(unlisten);

    const eventManager = createEventManager();
    await eventManager.registerAll([
      { name: 'test-event', handler: vi.fn() },
    ]);

    eventManager.cleanup();
    eventManager.cleanup();

    expect(unlisten).toHaveBeenCalledTimes(1);
  });

  it('registerAll after cleanup is a no-op', async () => {
    const eventManager = createEventManager();
    eventManager.cleanup();

    await eventManager.registerAll([
      { name: 'test-event', handler: vi.fn() },
    ]);

    expect(listenMock).not.toHaveBeenCalled();
  });
});
