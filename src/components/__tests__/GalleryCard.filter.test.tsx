/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { GalleryCard } from '../GalleryCard';
import type { MediaItemDto } from '../../types';
import { flush } from '../../test-utils/flush';
import { createMockRectObserver } from '../../test-utils/mock-resize-observer';
import { setupReactRoot } from '../../test-utils/react-root';

// ---- Fixtures ----

/** Epoch-ms for a Y/M/D at local noon (robust to DST shifts). */
function msFor(y: number, m: number, d: number): number {
  return new Date(y, m - 1, d, 12, 0, 0).getTime();
}

const D_DAY1 = msFor(2026, 7, 19);
const D_DAY2 = msFor(2026, 7, 18);

/** A gallery with one item per requested extension, spread across two days. */
function buildItems(): MediaItemDto[] {
  return [
    { mediaId: 'jpg-a', uri: 'content://jpg-a', dateModifiedMs: D_DAY1, width: 1, height: 1, mimeType: null, displayName: null, filePath: '/sdcard/DCIM/A.JPG' },
    { mediaId: 'heic-a', uri: 'content://heic-a', dateModifiedMs: D_DAY1, width: 1, height: 1, mimeType: null, displayName: null, filePath: '/sdcard/DCIM/B.HEIC' },
    { mediaId: 'cr3-a', uri: 'content://cr3-a', dateModifiedMs: D_DAY1, width: 1, height: 1, mimeType: null, displayName: null, filePath: '/sdcard/DCIM/C.CR3' },
    { mediaId: 'png-a', uri: 'content://png-a', dateModifiedMs: D_DAY2, width: 1, height: 1, mimeType: null, displayName: null, filePath: '/sdcard/DCIM/D.PNG' },
  ];
}

// ---- Mocks ----

let mockItems = buildItems();
const mockReload = vi.fn(async () => {});

vi.mock('../../hooks/useGalleryPager', () => ({
  useGalleryPager: () => ({
    get items() { return mockItems; },
    cursor: null,
    totalCount: mockItems.length,
    isLoading: false,
    error: null,
    loadNextPage: vi.fn(),
    reload: mockReload,
    loadAll: vi.fn().mockResolvedValue(undefined),
    removeItems: vi.fn(),
    addItems: vi.fn(),
  }),
}));

vi.mock('../../hooks/useThumbnailScheduler', () => ({
  useThumbnailScheduler: () => ({
    thumbnails: new Map<string, string>(),
    loadingThumbs: new Set<string>(),
    updateViewport: vi.fn(),
    removeThumbs: vi.fn(),
    cleanup: vi.fn(),
    registerMedia: vi.fn(),
  }),
}));

vi.mock('../../hooks/useGallerySelection', () => ({
  useGallerySelection: () => ({
    isSelectionMode: false,
    selectedIds: new Set<string>(),
    showMenu: false,
    deletingIds: new Set<string>(),
    menuRef: { current: null },
    handleTouchStart: vi.fn(),
    handleTouchEnd: vi.fn(),
    handleSelectionClick: vi.fn(() => false),
    handleRefreshStart: vi.fn(),
    handleDelete: vi.fn(),
    handleShare: vi.fn(),
    handleCancelSelection: vi.fn(),
    toggleMenu: vi.fn(),
    isDragSelectingRef: { current: false },
    dragAnchorIndexRef: { current: -1 },
    handleDragSelect: vi.fn(),
  }),
}));

vi.mock('../../hooks/useImagePreviewOpener', () => ({
  useImagePreviewOpener: () => vi.fn().mockResolvedValue(undefined),
}));

vi.mock('../../hooks/useAndroidAutoOpenLatestPhoto', () => ({
  useAndroidAutoOpenLatestPhoto: () => {},
}));

vi.mock('../../services/gallery-media-v2', () => ({
  isGalleryV2Available: () => true,
  invalidateMediaIds: vi.fn().mockResolvedValue(undefined),
}));

vi.mock('../../stores/configStore', () => {
  const state = { activeTab: 'gallery' };
  return {
    useConfigStore: (selector?: (s: typeof state) => unknown) =>
      selector ? selector(state) : state,
    useDraftConfig: () => null,
  };
});

vi.mock('../../stores/permissionStore', () => {
  const state = { requestStoragePermission: vi.fn(), startPolling: vi.fn() };
  return {
    usePermissionStore: (selector?: (s: typeof state) => unknown) =>
      selector ? selector(state) : state,
  };
});

// ---- Helpers ----

function filterButton(container: HTMLElement, mode: string): HTMLButtonElement | null {
  return container.querySelector(`[data-testid="gallery-filter-${mode}"]`) as HTMLButtonElement | null;
}

function activeFilterMode(container: HTMLElement): string | null {
  const active = container.querySelector('[data-testid^="gallery-filter-"].bg-blue-500') as HTMLButtonElement | null;
  return active?.getAttribute('data-testid')?.replace('gallery-filter-', '') ?? null;
}

function cellIds(container: HTMLElement): string[] {
  return Array.from(container.querySelectorAll('[data-media-id]')).map((el) => (el as HTMLElement).dataset.mediaId!);
}

// ---- Tests ----

describe('GalleryCard extension filter', () => {
  const { getContainer, getRoot } = setupReactRoot();
  let originalResizeObserver: typeof ResizeObserver;

  beforeEach(() => {
    const resizeMock = createMockRectObserver();
    originalResizeObserver = window.ResizeObserver;
    window.ResizeObserver = resizeMock.MockResizeObserver as unknown as typeof ResizeObserver;
    mockItems = buildItems();
    mockReload.mockClear();
  });

  afterEach(() => {
    window.ResizeObserver = originalResizeObserver;
  });

  it('renders only filter buttons for categories that have files, plus 全部', async () => {
    await act(async () => {
      getRoot().render(<GalleryCard />);
      await flush();
    });
    const c = getContainer();

    // jpg, heic, cr3, png present → all four categories + "全部".
    expect(filterButton(c, 'all')).toBeTruthy();
    expect(filterButton(c, 'jpeg')).toBeTruthy();
    expect(filterButton(c, 'heif')).toBeTruthy();
    expect(filterButton(c, 'raw')).toBeTruthy();
    expect(filterButton(c, 'other')).toBeTruthy();
  });

  it('defaults to 全部 and shows every file', async () => {
    await act(async () => {
      getRoot().render(<GalleryCard />);
      await flush();
    });
    const c = getContainer();

    expect(activeFilterMode(c)).toBe('all');
    expect(cellIds(c).sort()).toEqual(['cr3-a', 'heic-a', 'jpg-a', 'png-a']);
  });

  it('hides a category button when no file of that type exists', async () => {
    // Drop the only HEIC file → heif category should disappear.
    mockItems = mockItems.filter((i) => i.mediaId !== 'heic-a');
    await act(async () => {
      getRoot().render(<GalleryCard />);
      await flush();
    });
    const c = getContainer();

    expect(filterButton(c, 'heif')).toBeNull();
    expect(filterButton(c, 'jpeg')).toBeTruthy();
  });

  it('filters the grid to the selected category on click', async () => {
    await act(async () => {
      getRoot().render(<GalleryCard />);
      await flush();
    });
    const c = getContainer();

    await act(async () => {
      filterButton(c, 'jpeg')!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    expect(activeFilterMode(c)).toBe('jpeg');
    expect(cellIds(c)).toEqual(['jpg-a']);
  });

  it('keeps date-jump working under a filter (filtered index, filtered dates)', async () => {
    await act(async () => {
      getRoot().render(<GalleryCard />);
      await flush();
    });
    const c = getContainer();

    // Restrict to "other" (png-a on 2026-7-18).
    await act(async () => {
      filterButton(c, 'other')!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    // Open the date picker — it must reflect only the filtered set.
    await act(async () => {
      c.querySelector('[data-testid="gallery-date-title"]')!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    const dialog = c.querySelector('[data-testid="date-jump-dialog"]');
    expect(dialog).toBeTruthy();
    const rows = Array.from(dialog!.querySelectorAll('[data-date-key]'));
    // Only the png day remains.
    expect(rows.length).toBe(1);
    expect(rows[0].getAttribute('data-date-key')).toBe('2026-7-18');

    // jsdom lacks Element.scrollTo; stub it before the jump triggers one.
    const gridContainer = c.querySelector('[data-testid="virtual-grid-container"]') as HTMLDivElement;
    let capturedTop = -1;
    gridContainer!.scrollTo = ((opts: { top?: number }) => { capturedTop = opts?.top ?? -1; }) as Element['scrollTo'];

    await act(async () => {
      (rows[0] as HTMLButtonElement).dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    // png-a is the sole filtered item → index 0 → row 0 → scrollTop 0.
    expect(capturedTop).toBe(0);
    expect(c.querySelector('[data-testid="date-jump-dialog"]')).toBeNull();
  });

  it('resets the filter to 全部 when the refresh button is tapped', async () => {
    vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout', 'Date'] });
    try {
      await act(async () => {
        getRoot().render(<GalleryCard />);
        await flush();
      });
      const c = getContainer();

      // Activate the RAW filter first.
      await act(async () => {
        filterButton(c, 'raw')!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
        await flush();
      });
      expect(activeFilterMode(c)).toBe('raw');

      // Tap refresh. handleRefresh wraps reload() in withMinDuration (200ms),
      // so drain the timer before asserting the post-refresh state.
      await act(async () => {
        c.querySelector('[data-testid="refresh-button"]')!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
        await vi.advanceTimersByTimeAsync(300);
        await flush();
      });

      expect(mockReload).toHaveBeenCalledTimes(1);
      expect(activeFilterMode(c)).toBe('all');
      expect(cellIds(c).sort()).toEqual(['cr3-a', 'heic-a', 'jpg-a', 'png-a']);
    } finally {
      vi.useRealTimers();
    }
  });
});
