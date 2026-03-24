/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { LATEST_PHOTO_REFRESH_REQUESTED_EVENT } from '../../utils/gallery-refresh';
import { __resetLatestPhotoStoreForTests, useLatestPhoto } from '../useLatestPhoto';

const { listenMock, fetchLatestPhotoFileMock } = vi.hoisted(() => ({
  listenMock: vi.fn(),
  fetchLatestPhotoFileMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: listenMock,
}));

vi.mock('../../services/latest-photo', () => ({
  fetchLatestPhotoFile: fetchLatestPhotoFileMock,
}));

interface FileIndexChangedEvent {
  count: number;
  latestFilename: string | null;
}

function LatestPhotoHarness() {
  const { latestPhoto, refreshLatestPhoto } = useLatestPhoto();

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

async function flush(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}

describe('useLatestPhoto', () => {
  let container: HTMLDivElement;
  let root: Root;
  let fileIndexChangedHandler: ((event: { payload: FileIndexChangedEvent }) => void) | null;

  beforeEach(() => {
    __resetLatestPhotoStoreForTests();
    vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);

    fileIndexChangedHandler = null;
    listenMock.mockReset();
    fetchLatestPhotoFileMock.mockReset();
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
    __resetLatestPhotoStoreForTests();
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

  it('deduplicates in-flight refreshes across multiple consumers and events', async () => {
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

    expect(fetchLatestPhotoFileMock).toHaveBeenCalledTimes(1);

    await act(async () => {
      resolveFetch?.({
        filename: 'from-event.jpg',
        path: 'content://from-event',
      });
      await flush();
    });

    expect(fetchLatestPhotoFileMock).toHaveBeenCalledTimes(1);
    const filenames = Array.from(container.querySelectorAll('[data-testid="filename"]')).map(
      (node) => node.textContent,
    );
    expect(filenames).toEqual(['from-event.jpg', 'from-event.jpg']);
  });
});
