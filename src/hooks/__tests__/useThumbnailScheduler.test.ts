/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { describe, expect, it, vi, beforeEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import { useThumbnailScheduler } from '../useThumbnailScheduler';
import {
  enqueueThumbnails,
  cancelThumbnailRequests,
  registerThumbnailListener,
  unregisterThumbnailListener,
} from '../../services/gallery-media-v2';
import type { ThumbRequest, ThumbResult } from '../../types';

vi.mock('@tauri-apps/api/core', async () => {
  const actual = await vi.importActual('@tauri-apps/api/core');
  return {
    ...actual,
    convertFileSrc: (path: string) => `asset://localhost${path}`,
    invoke: vi.fn().mockResolvedValue(null),
  };
});

/** Typed reference to the mocked invoke for per-test overrides. */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
let mockInvoke: ReturnType<typeof vi.fn>;

vi.mock('../../services/gallery-media-v2', () => ({
  enqueueThumbnails: vi.fn().mockResolvedValue(undefined),
  cancelThumbnailRequests: vi.fn().mockResolvedValue(undefined),
  registerThumbnailListener: vi.fn().mockResolvedValue(undefined),
  unregisterThumbnailListener: vi.fn().mockResolvedValue(undefined),
}));

/** Short debounce for fast tests */
const TEST_DEBOUNCE = 2;

function getRegisteredListener(): (result: ThumbResult) => void {
  const calls = vi.mocked(registerThumbnailListener).mock.calls;
  const lastCall = calls[calls.length - 1];
  return lastCall[2] as (result: ThumbResult) => void;
}

function makeMedia(mediaId: string, dateModifiedMs = 1000) {
  return { mediaId, uri: `content://media/${mediaId}`, dateModifiedMs, filePath: null };
}

function makeReadyResult(requestId: string, mediaId: string, localPath: string): ThumbResult {
  return { requestId, mediaId, status: 'ready', localPath };
}

function makeFailedResult(
  requestId: string,
  mediaId: string,
  errorCode: string,
): ThumbResult {
  return { requestId, mediaId, status: 'failed', errorCode: errorCode as ThumbResult['errorCode'] };
}

/** Wait for the debounce timer to fire */
async function flushDebounce() {
  await act(async () => {
    await new Promise((r) => setTimeout(r, TEST_DEBOUNCE + 10));
  });
}

describe('useThumbnailScheduler', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockInvoke = vi.mocked(invoke);
    mockInvoke.mockResolvedValue(null);
  });

  it('enqueues visible items as high priority after debounce', async () => {
    const { result } = renderHook(() => useThumbnailScheduler({ debounceMs: TEST_DEBOUNCE }));

    act(() => {
      result.current.registerMedia([makeMedia('1'), makeMedia('2')]);
    });

    act(() => {
      result.current.updateViewport(['1', '2'], []);
    });

    // Debounce not yet fired
    expect(enqueueThumbnails).not.toHaveBeenCalled();

    await flushDebounce();

    expect(enqueueThumbnails).toHaveBeenCalledTimes(1);
    const reqs = vi.mocked(enqueueThumbnails).mock.calls[0][0] as ThumbRequest[];
    expect(reqs).toHaveLength(2);
    expect(reqs[0].mediaId).toBe('1');
    expect(reqs[0].priority).toBe('visible');
    expect(reqs[1].mediaId).toBe('2');
    expect(reqs[1].priority).toBe('visible');
  });

  it('enqueues nearby items as medium priority', async () => {
    const { result } = renderHook(() => useThumbnailScheduler({ debounceMs: TEST_DEBOUNCE }));

    act(() => {
      result.current.registerMedia([makeMedia('1'), makeMedia('2'), makeMedia('3')]);
    });

    act(() => {
      result.current.updateViewport(['1'], ['2', '3']);
    });

    await flushDebounce();

    const reqs = vi.mocked(enqueueThumbnails).mock.calls[0][0] as ThumbRequest[];
    expect(reqs).toHaveLength(3);
    expect(reqs.find((r) => r.mediaId === '1')?.priority).toBe('visible');
    expect(reqs.find((r) => r.mediaId === '2')?.priority).toBe('nearby');
    expect(reqs.find((r) => r.mediaId === '3')?.priority).toBe('nearby');
  });

  it('cancels requests that left both visible and nearby', async () => {
    const { result } = renderHook(() => useThumbnailScheduler({ debounceMs: TEST_DEBOUNCE }));

    act(() => {
      result.current.registerMedia([makeMedia('1'), makeMedia('2'), makeMedia('3')]);
    });

    // Initial viewport: all three
    act(() => {
      result.current.updateViewport(['1', '2', '3'], []);
    });
    await flushDebounce();

    expect(enqueueThumbnails).toHaveBeenCalledTimes(1);

    // Scroll: only '1' visible, '2' nearby, '3' is gone
    act(() => {
      result.current.updateViewport(['1'], ['2']);
    });
    await flushDebounce();

    expect(cancelThumbnailRequests).toHaveBeenCalledTimes(1);
    const cancelledIds = vi.mocked(cancelThumbnailRequests).mock.calls[0][0];
    expect(cancelledIds.length).toBeGreaterThan(0);
  });

  it('processes thumbnail result and updates thumbnails map', async () => {
    const { result } = renderHook(() => useThumbnailScheduler({ debounceMs: TEST_DEBOUNCE }));

    act(() => {
      result.current.registerMedia([makeMedia('1')]);
    });

    act(() => {
      result.current.updateViewport(['1'], []);
    });
    await flushDebounce();

    const reqs = vi.mocked(enqueueThumbnails).mock.calls[0][0] as ThumbRequest[];
    const req = reqs[0];

    const listener = getRegisteredListener();
    await act(async () => {
      listener(makeReadyResult(req.requestId, '1', '/cache/thumb_1.jpg'));
    });

    expect(result.current.thumbnails.get('1')).toBe('asset://localhost/cache/thumb_1.jpg');
    expect(result.current.loadingThumbs.has('1')).toBe(false);
  });

  it('rejects stale results where wantedKey no longer matches', async () => {
    const { result } = renderHook(() => useThumbnailScheduler({ debounceMs: TEST_DEBOUNCE }));

    act(() => {
      result.current.registerMedia([makeMedia('1', 1000)]);
    });

    act(() => {
      result.current.updateViewport(['1'], []);
    });
    await flushDebounce();

    const reqs = vi.mocked(enqueueThumbnails).mock.calls[0][0] as ThumbRequest[];
    const oldReq = reqs[0];

    // Simulate the media being updated (dateModifiedMs changed)
    act(() => {
      result.current.registerMedia([makeMedia('1', 2000)]);
    });

    // Re-enqueue with new metadata
    act(() => {
      result.current.updateViewport(['1'], []);
    });
    await flushDebounce();

    // Deliver result for the OLD request (stale wantedKey)
    const listener = getRegisteredListener();
    await act(async () => {
      listener(makeReadyResult(oldReq.requestId, '1', '/cache/thumb_old.jpg'));
    });

    // Should NOT be accepted because wantedKey changed
    expect(result.current.thumbnails.has('1')).toBe(false);
  });

  describe('retry logic by errorCode × priority matrix', () => {
    const retryableErrors = ['io_transient', 'oom_guard'];
    const permanentErrors = ['decode_corrupt', 'permission_denied', 'cancelled'];

    function setupFailedRequest(_errorCode: string) {
      const { result } = renderHook(() => useThumbnailScheduler({ debounceMs: TEST_DEBOUNCE }));

      act(() => {
        result.current.registerMedia([makeMedia('1')]);
      });

      act(() => {
        result.current.updateViewport(['1'], []);
      });

      return { result };
    }

    it.each(retryableErrors)('retries on %s (transient error)', async (errorCode) => {
      const { result } = setupFailedRequest(errorCode);
      await flushDebounce();

      const reqs = vi.mocked(enqueueThumbnails).mock.calls[0][0] as ThumbRequest[];
      const req = reqs[0];

      const listener = getRegisteredListener();
      await act(async () => {
        listener(makeFailedResult(req.requestId, '1', errorCode));
      });

      // After failure, loadingThumbs should be cleared
      expect(result.current.loadingThumbs.has('1')).toBe(false);

      // Re-trigger viewport to re-enqueue
      act(() => {
        result.current.updateViewport(['1'], []);
      });
      await flushDebounce();

      // Should have been enqueued again (2nd call)
      expect(enqueueThumbnails).toHaveBeenCalledTimes(2);
    });

    it.each(permanentErrors)('does NOT retry on %s (permanent error)', async (errorCode) => {
      const { result } = setupFailedRequest(errorCode);
      await flushDebounce();

      const reqs = vi.mocked(enqueueThumbnails).mock.calls[0][0] as ThumbRequest[];
      const req = reqs[0];

      const listener = getRegisteredListener();
      await act(async () => {
        listener(makeFailedResult(req.requestId, '1', errorCode));
      });

      expect(result.current.loadingThumbs.has('1')).toBe(false);

      // Re-trigger viewport
      act(() => {
        result.current.updateViewport(['1'], []);
      });
      await flushDebounce();

      // Should NOT have been re-enqueued (still only 1 call)
      expect(enqueueThumbnails).toHaveBeenCalledTimes(1);
    });
  });

  it('removeThumbs clears state and cancels active requests', async () => {
    const { result } = renderHook(() => useThumbnailScheduler({ debounceMs: TEST_DEBOUNCE }));

    act(() => {
      result.current.registerMedia([makeMedia('1'), makeMedia('2')]);
    });

    act(() => {
      result.current.updateViewport(['1', '2'], []);
    });
    await flushDebounce();

    expect(result.current.loadingThumbs.size).toBe(2);

    act(() => {
      result.current.removeThumbs(new Set(['1']));
    });

    expect(result.current.loadingThumbs.has('1')).toBe(false);
    expect(result.current.loadingThumbs.has('2')).toBe(true);
    expect(cancelThumbnailRequests).toHaveBeenCalled();
  });

  it('cleanup cancels all pending requests and clears loading state', async () => {
    const { result } = renderHook(() => useThumbnailScheduler({ debounceMs: TEST_DEBOUNCE }));

    act(() => {
      result.current.registerMedia([makeMedia('1'), makeMedia('2'), makeMedia('3')]);
    });

    act(() => {
      result.current.updateViewport(['1', '2', '3'], []);
    });
    await flushDebounce();

    expect(result.current.loadingThumbs.size).toBe(3);

    act(() => {
      result.current.cleanup();
    });

    expect(result.current.loadingThumbs.size).toBe(0);
    expect(cancelThumbnailRequests).toHaveBeenCalled();
  });

  it('does not duplicate cancellation work when cleanup is called before unmount', async () => {
    const { result, unmount } = renderHook(() => useThumbnailScheduler({ debounceMs: TEST_DEBOUNCE }));

    act(() => {
      result.current.registerMedia([makeMedia('1'), makeMedia('2')]);
    });

    act(() => {
      result.current.updateViewport(['1', '2'], []);
    });
    await flushDebounce();

    act(() => {
      result.current.cleanup();
    });
    unmount();

    expect(cancelThumbnailRequests).toHaveBeenCalledTimes(1);
  });

  it('debounces rapid viewport changes into a single enqueue', async () => {
    const { result } = renderHook(() => useThumbnailScheduler({ debounceMs: 50 }));

    act(() => {
      result.current.registerMedia([makeMedia('1'), makeMedia('2'), makeMedia('3')]);
    });

    // Rapid viewport changes within debounce window (all within 50ms)
    act(() => {
      result.current.updateViewport(['1'], []);
    });
    act(() => {
      result.current.updateViewport(['1', '2'], []);
    });
    act(() => {
      result.current.updateViewport(['2', '3'], []);
    });

    // Wait for debounce to fire
    await act(async () => {
      await new Promise((r) => setTimeout(r, 100));
    });

    // Only one enqueue call (last viewport state)
    expect(enqueueThumbnails).toHaveBeenCalledTimes(1);
    const reqs = vi.mocked(enqueueThumbnails).mock.calls[0][0] as ThumbRequest[];
    const mediaIds = reqs.map((r) => r.mediaId);
    expect(mediaIds).toContain('2');
    expect(mediaIds).toContain('3');
  });

  it('does not prefetch items beyond nearby range', async () => {
    const { result } = renderHook(() => useThumbnailScheduler({ debounceMs: TEST_DEBOUNCE }));

    act(() => {
      result.current.registerMedia([
        makeMedia('1'),
        makeMedia('2'),
        makeMedia('3'),
        makeMedia('4'),
      ]);
    });

    // Only '1' visible, '2' nearby — '3' and '4' are beyond range
    act(() => {
      result.current.updateViewport(['1'], ['2']);
    });
    await flushDebounce();

    const reqs = vi.mocked(enqueueThumbnails).mock.calls[0][0] as ThumbRequest[];
    const mediaIds = reqs.map((r) => r.mediaId);
    expect(mediaIds).toContain('1');
    expect(mediaIds).toContain('2');
    expect(mediaIds).not.toContain('3');
    expect(mediaIds).not.toContain('4');
  });

  it('registers and unregisters V2 listener on mount/unmount', () => {
    const LISTENER_ID = 'thumbnail-scheduler';
    const { unmount } = renderHook(() => useThumbnailScheduler({ debounceMs: TEST_DEBOUNCE }));

    expect(registerThumbnailListener).toHaveBeenCalledWith(
      'gallery-grid',
      LISTENER_ID,
      expect.any(Function),
    );

    unmount();

    expect(unregisterThumbnailListener).toHaveBeenCalledWith(LISTENER_ID);
  });

  describe('RAW orientation fix', () => {
    function makeRawMedia(mediaId: string, dateModifiedMs = 1000) {
      return {
        mediaId,
        uri: `content://media/${mediaId}`,
        dateModifiedMs,
        filePath: `/sdcard/DCIM/IMG_${mediaId}.nef`,
      };
    }

    /** Flush the debounce and resolve any pending async work from orientation fix. */
    async function flushOrientation() {
      await act(async () => {
        // Allow debounce + microtask queue to settle
        await new Promise((r) => setTimeout(r, TEST_DEBOUNCE + 20));
      });
    }

    it('calls get_raw_orientation for RAW files and injects EXIF when orientation > 1', async () => {
      mockInvoke.mockImplementation((cmd: string, _args?: Record<string, unknown>) => {
        if (cmd === 'get_raw_orientation') return Promise.resolve(6);
        if (cmd === 'inject_exif_orientation') return Promise.resolve(true);
        return Promise.resolve(null);
      });

      const { result } = renderHook(() => useThumbnailScheduler({ debounceMs: TEST_DEBOUNCE }));

      act(() => {
        result.current.registerMedia([makeRawMedia('raw1')]);
      });
      act(() => {
        result.current.updateViewport(['raw1'], []);
      });
      await flushOrientation();

      const reqs = vi.mocked(enqueueThumbnails).mock.calls[0][0] as ThumbRequest[];
      const req = reqs[0];

      const listener = getRegisteredListener();
      await act(async () => {
        listener(makeReadyResult(req.requestId, 'raw1', '/cache/thumb_raw1.jpg'));
        // Allow the async fixRawOrientation to settle
        await new Promise((r) => setTimeout(r, 10));
      });

      expect(mockInvoke).toHaveBeenCalledWith('get_raw_orientation', {
        filePath: '/sdcard/DCIM/IMG_raw1.nef',
      });
      expect(mockInvoke).toHaveBeenCalledWith('inject_exif_orientation', {
        thumbnailPath: '/cache/thumb_raw1.jpg',
        orientation: 6,
      });
      expect(result.current.thumbnails.get('raw1')).toBe('asset://localhost/cache/thumb_raw1.jpg');
      expect(result.current.loadingThumbs.has('raw1')).toBe(false);
    });

    it('skips inject_exif_orientation when orientation is 1 (no rotation needed)', async () => {
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'get_raw_orientation') return Promise.resolve(1);
        return Promise.resolve(null);
      });

      const { result } = renderHook(() => useThumbnailScheduler({ debounceMs: TEST_DEBOUNCE }));

      act(() => {
        result.current.registerMedia([makeRawMedia('raw2')]);
      });
      act(() => {
        result.current.updateViewport(['raw2'], []);
      });
      await flushOrientation();

      const reqs = vi.mocked(enqueueThumbnails).mock.calls[0][0] as ThumbRequest[];
      const req = reqs[0];

      const listener = getRegisteredListener();
      await act(async () => {
        listener(makeReadyResult(req.requestId, 'raw2', '/cache/thumb_raw2.jpg'));
        await new Promise((r) => setTimeout(r, 10));
      });

      expect(mockInvoke).toHaveBeenCalledWith('get_raw_orientation', {
        filePath: '/sdcard/DCIM/IMG_raw2.nef',
      });
      expect(mockInvoke).not.toHaveBeenCalledWith('inject_exif_orientation', expect.anything());
      expect(result.current.thumbnails.get('raw2')).toBe('asset://localhost/cache/thumb_raw2.jpg');
    });

    it('falls back to displaying thumbnail as-is when orientation read fails', async () => {
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'get_raw_orientation') return Promise.reject(new Error('nomexif failed'));
        return Promise.resolve(null);
      });

      const { result } = renderHook(() => useThumbnailScheduler({ debounceMs: TEST_DEBOUNCE }));

      act(() => {
        result.current.registerMedia([makeRawMedia('raw3')]);
      });
      act(() => {
        result.current.updateViewport(['raw3'], []);
      });
      await flushOrientation();

      const reqs = vi.mocked(enqueueThumbnails).mock.calls[0][0] as ThumbRequest[];
      const req = reqs[0];

      const listener = getRegisteredListener();
      await act(async () => {
        listener(makeReadyResult(req.requestId, 'raw3', '/cache/thumb_raw3.jpg'));
        await new Promise((r) => setTimeout(r, 10));
      });

      expect(mockInvoke).toHaveBeenCalledWith('get_raw_orientation', {
        filePath: '/sdcard/DCIM/IMG_raw3.nef',
      });
      // Thumbnail still displayed despite orientation failure
      expect(result.current.thumbnails.get('raw3')).toBe('asset://localhost/cache/thumb_raw3.jpg');
      expect(result.current.loadingThumbs.has('raw3')).toBe(false);
    });

    it('skips orientation fix entirely for non-RAW files', async () => {
      mockInvoke.mockResolvedValue(null);

      const { result } = renderHook(() => useThumbnailScheduler({ debounceMs: TEST_DEBOUNCE }));

      act(() => {
        result.current.registerMedia([makeMedia('jpg1')]);
      });
      act(() => {
        result.current.updateViewport(['jpg1'], []);
      });
      await flushOrientation();

      const reqs = vi.mocked(enqueueThumbnails).mock.calls[0][0] as ThumbRequest[];
      const req = reqs[0];

      const listener = getRegisteredListener();
      await act(async () => {
        listener(makeReadyResult(req.requestId, 'jpg1', '/cache/thumb_jpg1.jpg'));
      });

      expect(mockInvoke).not.toHaveBeenCalledWith('get_raw_orientation', expect.anything());
      expect(result.current.thumbnails.get('jpg1')).toBe('asset://localhost/cache/thumb_jpg1.jpg');
    });
  });
});
