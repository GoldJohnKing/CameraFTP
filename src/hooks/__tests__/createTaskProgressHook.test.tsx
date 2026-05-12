/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { act } from 'react';
import { describe, expect, it, vi, beforeEach } from 'vitest';
import { flush } from '../../test-utils/flush';
import { setupReactRoot } from '../../test-utils/react-root';

const { listenMock, capturedHandler } = vi.hoisted(() => {
  const captured: { current: ((payload: unknown) => void) | undefined } = { current: undefined };
  const mock = vi.fn().mockImplementation(async (_name: string, handler: (e: { payload: unknown }) => void) => {
    captured.current = (payload: unknown) => handler({ payload });
    return vi.fn();
  });
  return { listenMock: mock, capturedHandler: captured };
});

vi.mock('@tauri-apps/api/event', () => ({ listen: listenMock }));
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));
vi.mock('../../utils/gallery-refresh', () => ({
  requestMediaLibraryRefresh: vi.fn(),
  GALLERY_REFRESH_REQUESTED_EVENT: 'gallery-refresh-requested',
  LATEST_PHOTO_REFRESH_REQUESTED_EVENT: 'latest-photo-refresh-requested',
}));

import { createTaskProgressHook } from '../createTaskProgressHook';

interface TestEvent {
  type: 'progress' | 'done' | 'other';
  current?: number;
  total?: number;
  fileName?: string;
  failedCount?: number;
  failedFiles?: string[];
  outputFiles?: string[];
  cancelled?: boolean;
}

const testHook = createTaskProgressHook<TestEvent>({
  eventName: 'test-event',
  debugLabel: 'test-task',
  mapEvent: (event) => {
    switch (event.type) {
      case 'progress':
        return { type: 'progress', current: event.current!, total: event.total!, fileName: event.fileName ?? '', failedCount: event.failedCount ?? 0 };
      case 'done':
        return { type: 'done', total: event.total!, failedCount: event.failedCount ?? 0, failedFiles: event.failedFiles ?? [], outputFiles: event.outputFiles ?? [], cancelled: event.cancelled ?? false };
      default:
        return null;
    }
  },
});

function Harness() {
  const state = testHook.useProgress();
  return (
    <div>
      <span data-testid="is-active">{state.isActive ? 'yes' : 'no'}</span>
      <span data-testid="is-done">{state.isDone ? 'yes' : 'no'}</span>
      <span data-testid="current">{state.current}</span>
      <span data-testid="total">{state.total}</span>
    </div>
  );
}

describe('createTaskProgressHook', () => {
  const { getContainer, getRoot } = setupReactRoot();

  beforeEach(async () => {
    testHook.dismissDone();
    await act(async () => {
      getRoot().render(<Harness />);
      await flush();
    });
  });

  function getText(testId: string) {
    return getContainer().querySelector(`[data-testid="${testId}"]`)?.textContent ?? '';
  }

  it('starts in initial state', () => {
    expect(getText('is-active')).toBe('no');
    expect(getText('is-done')).toBe('no');
    expect(getText('current')).toBe('0');
  });

  it('handles progress event', async () => {
    await act(async () => {
      capturedHandler.current!({ type: 'progress', current: 2, total: 5, fileName: 'test.nef', failedCount: 0 });
      await flush();
    });
    expect(getText('is-active')).toBe('yes');
    expect(getText('current')).toBe('2');
    expect(getText('total')).toBe('5');
  });

  it('handles done event', async () => {
    await act(async () => {
      capturedHandler.current!({ type: 'done', total: 3, failedCount: 0, failedFiles: [], outputFiles: ['/out.jpg'], cancelled: false });
      await flush();
    });
    expect(getText('is-active')).toBe('no');
    expect(getText('is-done')).toBe('yes');
    expect(getText('current')).toBe('3');
  });

  it('dismissDone resets to initial state', async () => {
    await act(async () => {
      capturedHandler.current!({ type: 'done', total: 1, failedCount: 0, failedFiles: [], outputFiles: [], cancelled: false });
      await flush();
    });
    expect(getText('is-done')).toBe('yes');

    testHook.dismissDone();
    await act(async () => { await flush(); });

    expect(getText('is-done')).toBe('no');
    expect(getText('current')).toBe('0');
  });

  it('cancelled done resets to initial state', async () => {
    await act(async () => {
      capturedHandler.current!({ type: 'progress', current: 1, total: 2, fileName: 'a.nef', failedCount: 0 });
      await flush();
    });
    expect(getText('is-active')).toBe('yes');

    await act(async () => {
      capturedHandler.current!({ type: 'done', total: 2, failedCount: 0, failedFiles: [], outputFiles: ['/out.jpg'], cancelled: true });
      await flush();
    });
    expect(getText('is-active')).toBe('no');
    expect(getText('is-done')).toBe('no');
    expect(getText('current')).toBe('0');
  });

  it('getProgressState returns current snapshot', async () => {
    await act(async () => {
      capturedHandler.current!({ type: 'progress', current: 3, total: 10, fileName: 'a.nef', failedCount: 1 });
      await flush();
    });
    const state = testHook.getProgressState();
    expect(state.isActive).toBe(true);
    expect(state.current).toBe(3);
    expect(state.total).toBe(10);
    expect(state.failedCount).toBe(1);
  });

  it('unknown event type is ignored', async () => {
    await act(async () => {
      capturedHandler.current!({ type: 'other' } as TestEvent);
      await flush();
    });
    expect(getText('is-active')).toBe('no');
    expect(getText('is-done')).toBe('no');
  });
});
