/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { act } from 'react';
import { beforeEach, afterEach, describe, expect, it, vi } from 'vitest';
import type { ColorGradingEvent } from '../../types';
import { flush } from '../../test-utils/flush';
import { setupReactRoot } from '../../test-utils/react-root';

const {
  listenMock,
  invokeMock,
  requestMediaLibraryRefreshMock,
  capturedHandler,
} = vi.hoisted(() => {
  const _captured: { current: ((payload: ColorGradingEvent) => void) | undefined } = { current: undefined };
  const _listenMock = vi.fn().mockImplementation(async (
    _name: string,
    handler: (e: { payload: ColorGradingEvent }) => void,
  ) => {
    _captured.current = (payload: ColorGradingEvent) => handler({ payload });
    return vi.fn();
  });
  return {
    listenMock: _listenMock,
    invokeMock: vi.fn(),
    requestMediaLibraryRefreshMock: vi.fn(),
    capturedHandler: _captured,
  };
});

vi.mock('@tauri-apps/api/event', () => ({
  listen: listenMock,
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

vi.mock('../../utils/gallery-refresh', () => ({
  requestMediaLibraryRefresh: requestMediaLibraryRefreshMock,
  GALLERY_REFRESH_REQUESTED_EVENT: 'gallery-refresh-requested',
  LATEST_PHOTO_REFRESH_REQUESTED_EVENT: 'latest-photo-refresh-requested',
}));

import { useColorGradingProgress, dismissColorGradingDone, cancelColorGrading, enqueueColorGrading } from '../useColorGradingProgress';

function Harness() {
  const state = useColorGradingProgress();
  return (
    <div>
      <span data-testid="is-processing">{state.isProcessing ? 'yes' : 'no'}</span>
      <span data-testid="is-done">{state.isDone ? 'yes' : 'no'}</span>
      <span data-testid="current">{state.current}</span>
      <span data-testid="total">{state.total}</span>
      <span data-testid="failed-count">{state.failedCount}</span>
      <span data-testid="failed-files">{state.failedFiles.join(',')}</span>
    </div>
  );
}

describe('useColorGradingProgress', () => {
  const { getContainer, getRoot } = setupReactRoot();
  let eventHandler: ((payload: ColorGradingEvent) => void) | undefined;

  beforeEach(async () => {
    requestMediaLibraryRefreshMock.mockClear();
    invokeMock.mockClear();
    window.ImageViewerAndroid = undefined;
    dismissColorGradingDone();

    await act(async () => {
      getRoot().render(<Harness />);
      await flush();
    });

    eventHandler = capturedHandler.current;
  });

  afterEach(() => {
    window.ImageViewerAndroid = undefined;
  });

  function doneEvent(overrides: Partial<Extract<ColorGradingEvent, { type: 'done' }>> = {}): ColorGradingEvent {
    return {
      type: 'done',
      total: 2,
      failedCount: 0,
      failedFiles: [],
      outputFiles: ['/tmp/out1.jpg', '/tmp/out2.jpg'],
      cancelled: false,
      ...overrides,
    };
  }

  function getText(testId: string): string {
    return getContainer().querySelector(`[data-testid="${testId}"]`)?.textContent ?? '';
  }

  it('handleEvent "done" triggers gallery refresh', async () => {
    vi.useFakeTimers();

    eventHandler!(doneEvent());
    await act(async () => { await flush(); });

    await act(async () => {
      vi.advanceTimersByTime(500);
      await flush();
    });

    expect(requestMediaLibraryRefreshMock).toHaveBeenCalledWith({ reason: 'color-grading' });

    vi.useRealTimers();
  });

  it('handleEvent "done" scans output files via Android bridge', () => {
    const scanNewFile = vi.fn();
    window.ImageViewerAndroid = {
      openOrNavigateTo: vi.fn(),
      isAppVisible: vi.fn(),
      onExifResult: vi.fn(),
      onExifResultForPosition: vi.fn(),
      requestExifForPositions: vi.fn(),
      resolveFilePath: vi.fn(),
      scanNewFile,
    };

    eventHandler!(doneEvent());

    expect(scanNewFile).toHaveBeenCalledTimes(2);
    expect(scanNewFile).toHaveBeenCalledWith('/tmp/out1.jpg');
    expect(scanNewFile).toHaveBeenCalledWith('/tmp/out2.jpg');
  });

  it('handleEvent "done" with failures shows done state', async () => {
    eventHandler!(doneEvent({
      failedCount: 1,
      failedFiles: ['bad.nef'],
    }));

    await act(async () => { await flush(); });

    expect(getText('is-done')).toBe('yes');
    expect(getText('failed-count')).toBe('1');
    expect(getText('failed-files')).toBe('bad.nef');
    expect(getText('is-processing')).toBe('no');
  });

  it('handleEvent "done" with no failures shows done state persistently', async () => {
    eventHandler!(doneEvent());

    await act(async () => { await flush(); });

    expect(getText('is-done')).toBe('yes');

    // State persists — auto-reset is handled by the TaskProgressPanel, not the hook
    expect(getText('is-done')).toBe('yes');
  });

  it('handleEvent "done" with failures does not auto-reset', async () => {
    vi.useFakeTimers();

    eventHandler!(doneEvent({
      failedCount: 1,
      failedFiles: ['bad.nef'],
    }));

    await act(async () => { await flush(); });

    await act(async () => {
      vi.advanceTimersByTime(600);
      await flush();
    });

    expect(getText('is-done')).toBe('yes');
    expect(getText('failed-count')).toBe('1');

    vi.useRealTimers();
  });

  it('handleEvent "progress" updates state correctly', async () => {
    await act(async () => {
      eventHandler!({
        type: 'progress',
        current: 1,
        total: 1,
        fileName: 'photo.nef',
        failedCount: 0,
      });
      await flush();
    });

    expect(getText('current')).toBe('1');
    expect(getText('total')).toBe('1');
    expect(getText('is-processing')).toBe('yes');
  });

  it('handleEvent "done" with cancelled silently resets', async () => {
    const scanNewFile = vi.fn();
    window.ImageViewerAndroid = {
      openOrNavigateTo: vi.fn(),
      isAppVisible: vi.fn(),
      onExifResult: vi.fn(),
      onExifResultForPosition: vi.fn(),
      requestExifForPositions: vi.fn(),
      resolveFilePath: vi.fn(),
      scanNewFile,
    };

    eventHandler!(doneEvent({ cancelled: true }));

    expect(getText('is-processing')).toBe('no');
    expect(getText('is-done')).toBe('no');
    expect(getText('current')).toBe('0');

    expect(scanNewFile).toHaveBeenCalledTimes(2);
  });

  it('cancelColorGrading invokes cancel_color_grading command', async () => {
    await cancelColorGrading();
    expect(invokeMock).toHaveBeenCalledWith('cancel_color_grading');
  });

  it('enqueueColorGrading passes files to backend with meteringMode and evOffset', async () => {
    await enqueueColorGrading(['/tmp/a.nef', '/tmp/b.nef'], 'preset-1', 'highlight-safe', 0);
    expect(invokeMock).toHaveBeenCalledWith('enqueue_color_grading', {
      filePaths: ['/tmp/a.nef', '/tmp/b.nef'],
      lutId: 'preset-1',
      meteringMode: 'highlight-safe',
      evOffset: 0,
    });
  });
});
