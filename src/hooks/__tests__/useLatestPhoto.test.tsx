/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { LATEST_PHOTO_REFRESH_REQUESTED_EVENT } from '../../utils/gallery-refresh';
import { flush } from '../../test-utils/flush';

const { listenMock, fetchLatestPhotoFileMock, isGalleryV2AvailableMock } = vi.hoisted(() => ({
  listenMock: vi.fn(),
  fetchLatestPhotoFileMock: vi.fn(),
  isGalleryV2AvailableMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: listenMock,
}));

vi.mock('../../services/latest-photo', () => ({
  fetchLatestPhotoFile: fetchLatestPhotoFileMock,
}));

vi.mock('../../services/gallery-media-v2', () => ({
  isGalleryV2Available: isGalleryV2AvailableMock,
}));

interface FileIndexChangedEvent {
  count: number;
  latestFilename: string | null;
}

let useLatestPhotoRef: typeof import('../useLatestPhoto').useLatestPhoto;

function LatestPhotoHarness() {
  const { latestPhoto, refreshLatestPhoto } = useLatestPhotoRef();

  return (
    <div>
      <span data-testid="filename">{latestPhoto?.filename ?? 'none'}</span>
      <button data-testid="refresh" onClick={() => void refreshLatestPhoto()}>refresh</button>
    </div>
  );
}

function MultiConsumerHarness() {
  return (
    <div>
      <LatestPhotoHarness />
      <LatestPhotoHarness />
    </div>
  );
}

describe('useLatestPhoto', () => {
  let container: HTMLDivElement;
  let root: Root;
  let fileIndexChangedHandler: ((event: { payload: FileIndexChangedEvent }) => void) | null;
  beforeEach(async () => {
    vi.resetModules();
    ({ useLatestPhoto: useLatestPhotoRef } = await import('../useLatestPhoto'));
    vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);

    fileIndexChangedHandler = null;
    listenMock.mockReset();
    fetchLatestPhotoFileMock.mockReset();
    isGalleryV2AvailableMock.mockReset();
    isGalleryV2AvailableMock.mockReturnValue(false);
    fetchLatestPhotoFileMock.mockResolvedValue({
      filename: 'latest.jpg',
      path: 'content://latest',
    });
    listenMock.mockImplementation((eventName: string, handler: unknown) => {
      if (eventName === 'file-index-changed') {
        fileIndexChangedHandler = handler as (event: { payload: FileIndexChangedEvent }) => void;
      }
      return Promise.resolve(() => {});
    });
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    vi.unstubAllGlobals();
  });

  it('loads latest photo on mount and on refresh request events', async () => {
    await act(async () => {
      root.render(<LatestPhotoHarness />);
      await flush();
    });

    expect(fetchLatestPhotoFileMock).toHaveBeenCalledTimes(1);
    expect(container.querySelector('[data-testid="filename"]')?.textContent).toBe('latest.jpg');

    await act(async () => {
      window.dispatchEvent(new CustomEvent(LATEST_PHOTO_REFRESH_REQUESTED_EVENT));
      await flush();
    });

    expect(fetchLatestPhotoFileMock).toHaveBeenCalledTimes(2);
  });

  it('clears state when file index count becomes zero', async () => {
    await act(async () => {
      root.render(<LatestPhotoHarness />);
      await flush();
    });

    expect(fileIndexChangedHandler).not.toBeNull();

    await act(async () => {
      fileIndexChangedHandler?.({
        payload: { count: 0, latestFilename: null },
      });
      await flush();
    });

    expect(container.querySelector('[data-testid="filename"]')?.textContent).toBe('none');
  });

  it('refreshes latest photo when file index count is non-zero', async () => {
    await act(async () => {
      root.render(<LatestPhotoHarness />);
      await flush();
    });

    fetchLatestPhotoFileMock.mockResolvedValueOnce({
      filename: 'new-latest.jpg',
      path: 'content://new-latest',
    });

    await act(async () => {
      fileIndexChangedHandler?.({
        payload: { count: 1, latestFilename: 'new-latest.jpg' },
      });
      await flush();
    });

    expect(fetchLatestPhotoFileMock).toHaveBeenCalledTimes(2);
    expect(container.querySelector('[data-testid="filename"]')?.textContent).toBe('new-latest.jpg');
  });

  it('refreshes latest photo on gallery-items-added when Gallery V2 is available', async () => {
    isGalleryV2AvailableMock.mockReturnValue(true);

    await act(async () => {
      root.render(<LatestPhotoHarness />);
      await flush();
    });

    fetchLatestPhotoFileMock.mockResolvedValueOnce({
      filename: 'android-latest.jpg',
      path: 'content://android-latest',
    });

    await act(async () => {
      window.dispatchEvent(
        new CustomEvent('gallery-items-added', {
          detail: {
            items: [{ uri: 'content://android-latest' }],
            timestamp: Date.now(),
          },
        }),
      );
      await flush();
    });

    expect(fetchLatestPhotoFileMock).toHaveBeenCalledTimes(2);
    expect(container.querySelector('[data-testid="filename"]')?.textContent).toBe('android-latest.jpg');
  });

  it('ignores gallery-items-added when Gallery V2 is unavailable', async () => {
    const addEventListenerSpy = vi.spyOn(window, 'addEventListener');

    await act(async () => {
      root.render(<LatestPhotoHarness />);
      await flush();
    });

    await act(async () => {
      window.dispatchEvent(
        new CustomEvent('gallery-items-added', {
          detail: {
            items: [{ uri: 'content://ignored' }],
            timestamp: Date.now(),
          },
        }),
      );
      await flush();
    });

    expect(fetchLatestPhotoFileMock).toHaveBeenCalledTimes(1);
    expect(
      addEventListenerSpy.mock.calls.filter((call) => call[0] === 'gallery-items-added'),
    ).toHaveLength(0);
  });

  it('cleans up gallery-items-added listener on unmount when Gallery V2 is available', async () => {
    isGalleryV2AvailableMock.mockReturnValue(true);

    await act(async () => {
      root.render(<LatestPhotoHarness />);
      await flush();
    });

    expect(fetchLatestPhotoFileMock).toHaveBeenCalledTimes(1);

    await act(async () => {
      root.unmount();
      await flush();
    });
    root = createRoot(container);

    await act(async () => {
      window.dispatchEvent(
        new CustomEvent('gallery-items-added', {
          detail: {
            items: [{ uri: 'content://after-unmount' }],
            timestamp: Date.now(),
          },
        }),
      );
      await flush();
    });

    expect(fetchLatestPhotoFileMock).toHaveBeenCalledTimes(1);
  });

  it('uses singleton listeners and refresh for multiple consumers', async () => {
    const addEventListenerSpy = vi.spyOn(window, 'addEventListener');

    await act(async () => {
      root.render(<MultiConsumerHarness />);
      await flush();
    });

    expect(listenMock).toHaveBeenCalledTimes(1);
    expect(listenMock).toHaveBeenCalledWith('file-index-changed', expect.any(Function));
    expect(fetchLatestPhotoFileMock).toHaveBeenCalledTimes(1);
    expect(addEventListenerSpy).toHaveBeenCalledWith(
      LATEST_PHOTO_REFRESH_REQUESTED_EVENT,
      expect.any(Function),
    );
    expect(
      addEventListenerSpy.mock.calls.filter((call) => call[0] === LATEST_PHOTO_REFRESH_REQUESTED_EVENT),
    ).toHaveLength(1);

    await act(async () => {
      window.dispatchEvent(new CustomEvent(LATEST_PHOTO_REFRESH_REQUESTED_EVENT));
      await flush();
    });

    expect(fetchLatestPhotoFileMock).toHaveBeenCalledTimes(2);
    const filenames = Array.from(container.querySelectorAll('[data-testid="filename"]')).map(
      (node) => node.textContent,
    );
    expect(filenames).toEqual(['latest.jpg', 'latest.jpg']);
  });

  it('deduplicates in-flight refreshes and triggers follow-up when events arrive during fetch', async () => {
    let resolveFetch: ((value: { filename: string; path: string }) => void) | null = null;
    fetchLatestPhotoFileMock.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveFetch = resolve;
        }),
    );

    await act(async () => {
      root.render(<MultiConsumerHarness />);
      await Promise.resolve();
    });

    expect(fetchLatestPhotoFileMock).toHaveBeenCalledTimes(1);

    await act(async () => {
      const refreshButtons = container.querySelectorAll('[data-testid="refresh"]');
      (refreshButtons[0] as HTMLButtonElement).click();
      (refreshButtons[1] as HTMLButtonElement).click();
      window.dispatchEvent(new CustomEvent(LATEST_PHOTO_REFRESH_REQUESTED_EVENT));
      fileIndexChangedHandler?.({
        payload: { count: 2, latestFilename: 'from-event.jpg' },
      });
      await Promise.resolve();
    });

    // Still only 1 call — deduplicated while in-flight
    expect(fetchLatestPhotoFileMock).toHaveBeenCalledTimes(1);

    fetchLatestPhotoFileMock.mockResolvedValueOnce({
      filename: 'follow-up.jpg',
      path: 'content://follow-up',
    });

    await act(async () => {
      resolveFetch?.({
        filename: 'from-event.jpg',
        path: 'content://from-event',
      });
      await flush();
    });

    // Follow-up refresh triggered because events arrived during in-flight fetch
    expect(fetchLatestPhotoFileMock).toHaveBeenCalledTimes(2);
    const filenames = Array.from(container.querySelectorAll('[data-testid="filename"]')).map(
      (node) => node.textContent,
    );
    expect(filenames).toEqual(['follow-up.jpg', 'follow-up.jpg']);
  });
});
