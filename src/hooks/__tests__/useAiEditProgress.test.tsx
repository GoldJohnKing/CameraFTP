/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { act } from 'react';
import { beforeEach, afterEach, describe, expect, it, vi } from 'vitest';
import type { AiEditProgressEvent } from '../../types';
import { flush } from '../../test-utils/flush';
import { setupReactRoot } from '../../test-utils/react-root';

const {
  listenMock,
  invokeMock,
  requestMediaLibraryRefreshMock,
  openImagePreviewMock,
  capturedHandler,
} = vi.hoisted(() => {
  const _captured: { current: ((payload: AiEditProgressEvent) => void) | undefined } = { current: undefined };
  const _listenMock = vi.fn().mockImplementation(async (
    _name: string,
    handler: (e: { payload: AiEditProgressEvent }) => void,
  ) => {
    _captured.current = (payload: AiEditProgressEvent) => handler({ payload });
    return vi.fn();
  });
  return {
    listenMock: _listenMock,
    invokeMock: vi.fn(),
    requestMediaLibraryRefreshMock: vi.fn(),
    openImagePreviewMock: vi.fn(),
    capturedHandler: _captured,
  };
});

const mockConfigGetState = vi.fn().mockReturnValue({
  draft: {
    androidImageViewer: {
      autoOpenLatestWhenVisible: false,
      openMethod: 'built-in-viewer',
    },
  },
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

vi.mock('../../services/image-open', () => ({
  openImagePreview: openImagePreviewMock,
}));

vi.mock('../../stores/configStore', () => ({
  useConfigStore: { getState: mockConfigGetState },
}));

import { useAiEditProgress, dismissDone, useAiEditProgressListener, cancelAiEdit } from '../useAiEditProgress';

function Harness() {
  const state = useAiEditProgress();
  return (
    <div>
      <span data-testid="is-editing">{state.isEditing ? 'yes' : 'no'}</span>
      <span data-testid="is-done">{state.isDone ? 'yes' : 'no'}</span>
      <span data-testid="current">{state.current}</span>
      <span data-testid="total">{state.total}</span>
      <span data-testid="failed-count">{state.failedCount}</span>
      <span data-testid="failed-files">{state.failedFiles.join(',')}</span>
    </div>
  );
}

describe('useAiEditProgress', () => {
  const { getContainer, getRoot } = setupReactRoot();
  let eventHandler: ((payload: AiEditProgressEvent) => void) | undefined;

  beforeEach(async () => {
    requestMediaLibraryRefreshMock.mockClear();
    openImagePreviewMock.mockClear();
    mockConfigGetState.mockClear();
    mockConfigGetState.mockReturnValue({
      draft: {
        androidImageViewer: {
          autoOpenLatestWhenVisible: false,
          openMethod: 'built-in-viewer',
        },
      },
    });
    window.ImageViewerAndroid = undefined;
    dismissDone();
    eventHandler = capturedHandler.current;

    await act(async () => {
      getRoot().render(<Harness />);
      await flush();
    });
  });

  afterEach(() => {
    window.ImageViewerAndroid = undefined;
  });

  function doneEvent(overrides: Partial<Extract<AiEditProgressEvent, { type: 'done' }>> = {}): AiEditProgressEvent {
    return {
      type: 'done',
      total: 3,
      failedCount: 0,
      failedFiles: [],
      outputFiles: ['/tmp/out1.jpg', '/tmp/out2.jpg'],
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

    expect(requestMediaLibraryRefreshMock).toHaveBeenCalledWith({ reason: 'ai-edit' });

    vi.useRealTimers();
  });

  it('handleEvent "done" scans output files via Android bridge', () => {
    const scanNewFile = vi.fn();
    window.ImageViewerAndroid = {
      openOrNavigateTo: vi.fn(),
      isAppVisible: vi.fn(),
      onExifResult: vi.fn(),
      resolveFilePath: vi.fn(),
      scanNewFile,
    };

    eventHandler!(doneEvent());

    expect(scanNewFile).toHaveBeenCalledTimes(2);
    expect(scanNewFile).toHaveBeenCalledWith('/tmp/out1.jpg');
    expect(scanNewFile).toHaveBeenCalledWith('/tmp/out2.jpg');
  });

  it('handleEvent "done" notifies native layer on success', () => {
    const onAiEditComplete = vi.fn();
    window.ImageViewerAndroid = {
      openOrNavigateTo: vi.fn(),
      isAppVisible: vi.fn(),
      onExifResult: vi.fn(),
      resolveFilePath: vi.fn(),
      onAiEditComplete,
    };

    eventHandler!(doneEvent());

    expect(onAiEditComplete).toHaveBeenCalledWith(true, null);
  });

  it('handleEvent "done" notifies native layer with failure message', () => {
    const onAiEditComplete = vi.fn();
    window.ImageViewerAndroid = {
      openOrNavigateTo: vi.fn(),
      isAppVisible: vi.fn(),
      onExifResult: vi.fn(),
      resolveFilePath: vi.fn(),
      onAiEditComplete,
    };

    eventHandler!(doneEvent({
      failedCount: 2,
      failedFiles: ['fail1.jpg', 'fail2.jpg'],
    }));

    expect(onAiEditComplete).toHaveBeenCalledWith(
      false,
      '修图完成，2张失败：fail1.jpg、fail2.jpg',
    );
  });

  it('handleEvent "done" does not auto-preview when autoOpenLatestWhenVisible is false', async () => {
    mockConfigGetState.mockReturnValue({
      draft: {
        androidImageViewer: {
          autoOpenLatestWhenVisible: false,
          openMethod: 'built-in-viewer',
        },
      },
    });

    eventHandler!(doneEvent());
    await flush();

    expect(openImagePreviewMock).not.toHaveBeenCalled();
  });

  it('handleEvent "done" auto-previews when autoOpenLatestWhenVisible is true', async () => {
    mockConfigGetState.mockReturnValue({
      draft: {
        androidImageViewer: {
          autoOpenLatestWhenVisible: true,
          openMethod: 'built-in-viewer',
        },
      },
    });

    await act(async () => {
      eventHandler!(doneEvent());
      // Multiple flushes to resolve nested dynamic imports
      await flush();
      await flush();
      await flush();
      await flush();
    });

    expect(openImagePreviewMock).toHaveBeenCalledWith({
      filePath: '/tmp/out1.jpg',
      openMethod: 'built-in-viewer',
      allUris: ['/tmp/out1.jpg'],
    });
  });

  it('handleEvent "done" with failures shows done state', async () => {
    eventHandler!(doneEvent({
      failedCount: 1,
      failedFiles: ['bad.jpg'],
    }));

    await act(async () => { await flush(); });

    expect(getText('is-done')).toBe('yes');
    expect(getText('failed-count')).toBe('1');
    expect(getText('failed-files')).toBe('bad.jpg');
    expect(getText('is-editing')).toBe('no');
  });

  it('handleEvent "progress" syncs to native layer', () => {
    const updateAiEditProgress = vi.fn();
    window.ImageViewerAndroid = {
      openOrNavigateTo: vi.fn(),
      isAppVisible: vi.fn(),
      onExifResult: vi.fn(),
      resolveFilePath: vi.fn(),
      updateAiEditProgress,
    };

    eventHandler!({
      type: 'progress',
      current: 2,
      total: 5,
      fileName: 'photo.jpg',
      failedCount: 0,
    });

    expect(updateAiEditProgress).toHaveBeenCalledWith(2, 5, 0);
  });

  it('handleEvent "done" with no failures resets state after timeout', async () => {
    vi.useFakeTimers();

    eventHandler!(doneEvent());

    await act(async () => { await flush(); });

    expect(getText('is-editing')).toBe('no');
    expect(getText('is-done')).toBe('no');

    await act(async () => {
      vi.advanceTimersByTime(500);
      await flush();
    });

    expect(getText('current')).toBe('0');
    expect(getText('total')).toBe('0');
    expect(getText('failed-count')).toBe('0');

    vi.useRealTimers();
  });

  it('handleEvent "done" with failures does not auto-reset', async () => {
    vi.useFakeTimers();

    eventHandler!(doneEvent({
      failedCount: 1,
      failedFiles: ['bad.jpg'],
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

  it('useAiEditProgressListener does not register duplicate listener', async () => {
    listenMock.mockClear();

    function ListenerHarness() {
      useAiEditProgressListener();
      return null;
    }

    await act(async () => {
      getRoot().render(<ListenerHarness />);
      await flush();
      await flush();
    });

    // Already registered from module load, should NOT call listen again
    expect(listenMock).not.toHaveBeenCalled();
  });

  it('cancelAiEdit invokes cancel_ai_edit command', async () => {
    await cancelAiEdit();
    expect(invokeMock).toHaveBeenCalledWith('cancel_ai_edit');
  });

  it('handleEvent "done" triggers gallery refresh with ai-edit reason', async () => {
    vi.useFakeTimers();

    eventHandler!(doneEvent());
    await act(async () => { await flush(); });

    await act(async () => {
      vi.advanceTimersByTime(500);
      await flush();
    });

    expect(requestMediaLibraryRefreshMock).toHaveBeenCalledWith({ reason: 'ai-edit' });

    vi.useRealTimers();
  });

  it('handleEvent "progress" updates state correctly', async () => {
    await act(async () => {
      eventHandler!({
        type: 'progress',
        current: 1,
        total: 1,
        fileName: 'photo.jpg',
        failedCount: 0,
      });
      await flush();
    });

    expect(getText('current')).toBe('1');
    expect(getText('total')).toBe('1');
    expect(getText('is-editing')).toBe('yes');
  });

  it('handleEvent "queued" updates total when items are added during processing', async () => {
    // Simulate: first image starts processing
    await act(async () => {
      eventHandler!({
        type: 'progress',
        current: 1,
        total: 1,
        fileName: 'photo1.jpg',
        failedCount: 0,
      });
      await flush();
    });

    expect(getText('current')).toBe('1');
    expect(getText('total')).toBe('1');

    // Simulate: second image enqueued while first is still processing
    await act(async () => {
      eventHandler!({
        type: 'queued',
        queueDepth: 1,
      });
      await flush();
    });

    // total should now reflect the expanded queue: current(1) + queueDepth(1) = 2
    expect(getText('current')).toBe('1');
    expect(getText('total')).toBe('2');
  });

  it('handleEvent "queued" syncs updated total to native layer', () => {
    const updateAiEditProgress = vi.fn();
    window.ImageViewerAndroid = {
      openOrNavigateTo: vi.fn(),
      isAppVisible: vi.fn(),
      onExifResult: vi.fn(),
      resolveFilePath: vi.fn(),
      updateAiEditProgress,
    };

    // Start processing
    eventHandler!({
      type: 'progress',
      current: 1,
      total: 1,
      fileName: 'photo.jpg',
      failedCount: 0,
    });
    expect(updateAiEditProgress).toHaveBeenCalledWith(1, 1, 0);

    // New file queued
    eventHandler!({
      type: 'queued',
      queueDepth: 2,
    });
    expect(updateAiEditProgress).toHaveBeenCalledWith(1, 3, 0);
  });

  it('handleEvent "queued" is ignored when not editing', async () => {
    // No progress event sent — not in editing state
    await act(async () => {
      eventHandler!({
        type: 'queued',
        queueDepth: 1,
      });
      await flush();
    });

    // total should remain 0 (no editing session active)
    expect(getText('total')).toBe('0');
    expect(getText('is-editing')).toBe('no');
  });

  it('FTP upload scenario: total updates as images arrive during processing', async () => {
    // Image 1 starts processing
    await act(async () => {
      eventHandler!({
        type: 'progress',
        current: 1,
        total: 1,
        fileName: 'img1.jpg',
        failedCount: 0,
      });
      await flush();
    });
    expect(getText('current')).toBe('1');
    expect(getText('total')).toBe('1');

    // Image 2 arrives via FTP while image 1 is processing
    await act(async () => {
      eventHandler!({
        type: 'queued',
        queueDepth: 1,
      });
      await flush();
    });
    expect(getText('current')).toBe('1');
    expect(getText('total')).toBe('2');

    // Image 3 arrives via FTP
    await act(async () => {
      eventHandler!({
        type: 'queued',
        queueDepth: 2,
      });
      await flush();
    });
    expect(getText('current')).toBe('1');
    expect(getText('total')).toBe('3');

    // Image 1 completes
    await act(async () => {
      eventHandler!({
        type: 'completed',
        current: 1,
        total: 3,
        fileName: 'img1.jpg',
        failedCount: 0,
        outputPath: '/out/img1.jpg',
      });
      await flush();
    });

    // Image 2 starts processing
    await act(async () => {
      eventHandler!({
        type: 'progress',
        current: 2,
        total: 3,
        fileName: 'img2.jpg',
        failedCount: 0,
      });
      await flush();
    });
    expect(getText('current')).toBe('2');
    expect(getText('total')).toBe('3');
  });
});
