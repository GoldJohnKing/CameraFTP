/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useGalleryLibrary } from '../useGalleryLibrary';
import { GALLERY_REFRESH_REQUESTED_EVENT } from '../../utils/gallery-refresh';

const { requestStoragePermissionMock, startPollingMock } = vi.hoisted(() => ({
  requestStoragePermissionMock: vi.fn(),
  startPollingMock: vi.fn(),
}));

const { listGalleryMediaMock } = vi.hoisted(() => ({
  listGalleryMediaMock: vi.fn(),
}));

const { permissionBridgeMock, requestLatestPhotoRefreshMock } = vi.hoisted(() => ({
  permissionBridgeMock: {
    isAvailable: vi.fn(),
    checkAll: vi.fn(),
  },
  requestLatestPhotoRefreshMock: vi.fn(),
}));

vi.mock('../../stores/permissionStore', () => ({
  usePermissionStore: (selector: (state: {
    requestStoragePermission: () => void;
    startPolling: (mode?: 'all' | 'storage') => void;
  }) => unknown) => selector({
    requestStoragePermission: requestStoragePermissionMock,
    startPolling: startPollingMock,
  }),
}));

vi.mock('../../services/gallery-media', () => ({
  listGalleryMedia: listGalleryMediaMock,
}));

vi.mock('../../types', async () => {
  const actual = await vi.importActual<typeof import('../../types')>('../../types');
  return {
    ...actual,
    permissionBridge: permissionBridgeMock,
  };
});

vi.mock('../../utils/gallery-refresh', async () => {
  const actual = await vi.importActual<typeof import('../../utils/gallery-refresh')>('../../utils/gallery-refresh');
  return {
    ...actual,
    requestLatestPhotoRefresh: requestLatestPhotoRefreshMock,
  };
});

function GalleryLibraryHarness() {
  const { images, isLoading, isRefreshing, error, enteringIds, refresh, removeImages } = useGalleryLibrary();

  return (
    <div>
      <span data-testid="count">{images.length}</span>
      <span data-testid="loading">{isLoading ? 'yes' : 'no'}</span>
      <span data-testid="refreshing">{isRefreshing ? 'yes' : 'no'}</span>
      <span data-testid="error">{error ?? ''}</span>
      <span data-testid="entering-count">{enteringIds.size}</span>
      <button onClick={() => void refresh()} data-testid="refresh">refresh</button>
      <button onClick={() => removeImages(new Set(['content://2']))} data-testid="remove-content-2">remove-content-2</button>
      <button
        onClick={() => void refresh({ onStart: () => {
          const testWindow = window as Window & { __refreshStartCount?: number };
          testWindow.__refreshStartCount = (testWindow.__refreshStartCount ?? 0) + 1;
        } })}
        data-testid="refresh-with-on-start"
      >
        refresh-with-on-start
      </button>
    </div>
  );
}

async function flush(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}

describe('useGalleryLibrary', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);
    vi.useFakeTimers();

    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);

    requestStoragePermissionMock.mockReset();
    startPollingMock.mockReset();
    listGalleryMediaMock.mockReset();
    requestLatestPhotoRefreshMock.mockReset();
    permissionBridgeMock.isAvailable.mockReset();
    permissionBridgeMock.checkAll.mockReset();

    permissionBridgeMock.isAvailable.mockReturnValue(false);
    delete (window as Window & { __refreshStartCount?: number }).__refreshStartCount;
    window.GalleryAndroid = {} as typeof window.GalleryAndroid;
    listGalleryMediaMock.mockResolvedValue([
      { path: 'content://1', filename: '1.jpg', sortTime: 1 },
    ]);
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    vi.clearAllTimers();
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  it('loads gallery images on mount', async () => {
    await act(async () => {
      root.render(<GalleryLibraryHarness />);
      await flush();
    });

    expect(listGalleryMediaMock).toHaveBeenCalledTimes(1);
    expect(container.querySelector('[data-testid="count"]')?.textContent).toBe('1');
    expect(container.querySelector('[data-testid="loading"]')?.textContent).toBe('no');
  });

  it('reacts to gallery refresh requested event', async () => {
    await act(async () => {
      root.render(<GalleryLibraryHarness />);
      await flush();
    });

    await act(async () => {
      window.dispatchEvent(new CustomEvent(GALLERY_REFRESH_REQUESTED_EVENT));
      await flush();
    });

    expect(listGalleryMediaMock).toHaveBeenCalledTimes(2);
  });

  it('gates manual refresh when storage permission is missing', async () => {
    permissionBridgeMock.isAvailable.mockReturnValue(true);
    permissionBridgeMock.checkAll.mockResolvedValue({
      storage: false,
      notification: true,
      batteryOptimization: true,
    });

    await act(async () => {
      root.render(<GalleryLibraryHarness />);
      await flush();
    });

    await act(async () => {
      container.querySelector('[data-testid="refresh-with-on-start"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    expect(requestStoragePermissionMock).toHaveBeenCalledTimes(1);
    expect(startPollingMock).toHaveBeenCalledWith('storage');
    expect(requestLatestPhotoRefreshMock).not.toHaveBeenCalled();
    expect((window as Window & { __refreshStartCount?: number }).__refreshStartCount ?? 0).toBe(0);
  });

  it('runs refresh onStart callback when refresh actually starts', async () => {
    permissionBridgeMock.isAvailable.mockReturnValue(true);
    permissionBridgeMock.checkAll.mockResolvedValue({
      storage: true,
      notification: true,
      batteryOptimization: true,
    });

    await act(async () => {
      root.render(<GalleryLibraryHarness />);
      await flush();
    });

    await act(async () => {
      container.querySelector('[data-testid="refresh-with-on-start"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    expect((window as Window & { __refreshStartCount?: number }).__refreshStartCount).toBe(1);
  });

  it('tracks entering ids for newly added images', async () => {
    permissionBridgeMock.isAvailable.mockReturnValue(false);

    await act(async () => {
      root.render(<GalleryLibraryHarness />);
      await flush();
    });

    expect(container.querySelector('[data-testid="entering-count"]')?.textContent).toBe('0');

    listGalleryMediaMock.mockResolvedValueOnce([
      { path: 'content://1', filename: '1.jpg', sortTime: 1 },
      { path: 'content://2', filename: '2.jpg', sortTime: 2 },
    ]);

    await act(async () => {
      container.querySelector('[data-testid="refresh"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    expect(container.querySelector('[data-testid="entering-count"]')?.textContent).toBe('1');

    await act(async () => {
      vi.advanceTimersByTime(280);
      await flush();
    });

    expect(container.querySelector('[data-testid="entering-count"]')?.textContent).toBe('0');
  });

  it('keeps refresh spinner visible for minimum duration', async () => {
    permissionBridgeMock.isAvailable.mockReturnValue(false);

    await act(async () => {
      root.render(<GalleryLibraryHarness />);
      await flush();
    });

    await act(async () => {
      container.querySelector('[data-testid="refresh"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    expect(container.querySelector('[data-testid="refreshing"]')?.textContent).toBe('yes');
    expect(requestLatestPhotoRefreshMock).toHaveBeenCalledWith({ reason: 'manual' });

    await act(async () => {
      vi.advanceTimersByTime(199);
      await flush();
    });
    expect(container.querySelector('[data-testid="refreshing"]')?.textContent).toBe('yes');

    await act(async () => {
      vi.advanceTimersByTime(1);
      await flush();
    });
    expect(container.querySelector('[data-testid="refreshing"]')?.textContent).toBe('no');
  });

  it('updates previous path bookkeeping after local removeImages', async () => {
    listGalleryMediaMock.mockResolvedValueOnce([
      { path: 'content://1', filename: '1.jpg', sortTime: 1 },
      { path: 'content://2', filename: '2.jpg', sortTime: 2 },
    ]);

    await act(async () => {
      root.render(<GalleryLibraryHarness />);
      await flush();
    });

    expect(container.querySelector('[data-testid="count"]')?.textContent).toBe('2');

    await act(async () => {
      container.querySelector('[data-testid="remove-content-2"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    expect(container.querySelector('[data-testid="count"]')?.textContent).toBe('1');

    listGalleryMediaMock.mockResolvedValueOnce([
      { path: 'content://1', filename: '1.jpg', sortTime: 1 },
      { path: 'content://2', filename: '2.jpg', sortTime: 2 },
    ]);

    await act(async () => {
      container.querySelector('[data-testid="refresh"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    expect(container.querySelector('[data-testid="entering-count"]')?.textContent).toBe('1');
  });
});
