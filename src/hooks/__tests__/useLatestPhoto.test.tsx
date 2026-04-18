/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { act } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { LATEST_PHOTO_REFRESH_REQUESTED_EVENT } from '../../utils/gallery-refresh';
import { flush } from '../../test-utils/flush';
import { setupReactRoot } from '../../test-utils/react-root';

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

vi.mock('../../services/gallery-media-v2', () => ({
  isGalleryV2Available: vi.fn().mockReturnValue(false),
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
  const { getContainer, getRoot } = setupReactRoot();
  let fileIndexChangedHandler: ((event: { payload: FileIndexChangedEvent }) => void) | null;
  beforeEach(async () => {
    vi.resetModules();
    ({ useLatestPhoto: useLatestPhotoRef } = await import('../useLatestPhoto'));

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

  it('loads latest photo on mount', async () => {
    await act(async () => {
      getRoot().render(<LatestPhotoHarness />);
      await flush();
    });

    expect(fetchLatestPhotoFileMock).toHaveBeenCalledTimes(1);
    expect(
      getContainer().querySelector('[data-testid="filename"]')?.textContent,
    ).toBe('latest.jpg');
  });

  it('does NOT refresh on LATEST_PHOTO_REFRESH_REQUESTED_EVENT (only on click)', async () => {
    await act(async () => {
      getRoot().render(<LatestPhotoHarness />);
      await flush();
    });

    expect(fetchLatestPhotoFileMock).toHaveBeenCalledTimes(1);

    // Dispatching the refresh event should NOT trigger a fetch
    await act(async () => {
      window.dispatchEvent(new CustomEvent(LATEST_PHOTO_REFRESH_REQUESTED_EVENT));
      await flush();
    });

    // Still only 1 call — event does not trigger refresh
    expect(fetchLatestPhotoFileMock).toHaveBeenCalledTimes(1);
  });

  it('refreshes on manual click via refreshLatestPhoto', async () => {
    await act(async () => {
      getRoot().render(<LatestPhotoHarness />);
      await flush();
    });

    expect(fetchLatestPhotoFileMock).toHaveBeenCalledTimes(1);

    fetchLatestPhotoFileMock.mockResolvedValueOnce({
      filename: 'clicked-latest.jpg',
      path: 'content://clicked-latest',
    });

    // Click the refresh button
    await act(async () => {
      getContainer().querySelector('[data-testid="refresh"]')?.dispatchEvent(
        new MouseEvent('click', { bubbles: true }),
      );
      await flush();
    });

    expect(fetchLatestPhotoFileMock).toHaveBeenCalledTimes(2);
    expect(
      getContainer().querySelector('[data-testid="filename"]')?.textContent,
    ).toBe('clicked-latest.jpg');
  });

  it('clears state when file index count becomes zero', async () => {
    await act(async () => {
      getRoot().render(<LatestPhotoHarness />);
      await flush();
    });

    expect(fileIndexChangedHandler).not.toBeNull();

    await act(async () => {
      fileIndexChangedHandler?.({
        payload: { count: 0, latestFilename: null },
      });
      await flush();
    });

    expect(
      getContainer().querySelector('[data-testid="filename"]')?.textContent,
    ).toBe('none');
  });

  it('refreshes latest photo when file index count is non-zero', async () => {
    await act(async () => {
      getRoot().render(<LatestPhotoHarness />);
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
    expect(
      getContainer().querySelector('[data-testid="filename"]')?.textContent,
    ).toBe('new-latest.jpg');
  });

  it('uses singleton listeners for multiple consumers', async () => {
    await act(async () => {
      getRoot().render(<MultiConsumerHarness />);
      await flush();
    });

    expect(listenMock).toHaveBeenCalledTimes(1);
    expect(listenMock).toHaveBeenCalledWith('file-index-changed', expect.any(Function));
    expect(fetchLatestPhotoFileMock).toHaveBeenCalledTimes(1);
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
      getRoot().render(<MultiConsumerHarness />);
      await Promise.resolve();
    });

    expect(fetchLatestPhotoFileMock).toHaveBeenCalledTimes(1);

    await act(async () => {
      const refreshButtons = getContainer().querySelectorAll('[data-testid="refresh"]');
      (refreshButtons[0] as HTMLButtonElement).click();
      (refreshButtons[1] as HTMLButtonElement).click();
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
    const filenames = Array.from(
      getContainer().querySelectorAll('[data-testid="filename"]'),
    ).map((node) => node.textContent);
    expect(filenames).toEqual(['follow-up.jpg', 'follow-up.jpg']);
  });
});
