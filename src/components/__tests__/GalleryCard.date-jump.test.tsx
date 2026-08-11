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

// ---- Date helpers ----

const WEEKDAYS_ZH = ['周日', '周一', '周二', '周三', '周四', '周五', '周六'];

function titleFor(ms: number): string {
  const d = new Date(ms);
  return `${d.getFullYear()}年·${d.getMonth() + 1}月${d.getDate()}日·${WEEKDAYS_ZH[d.getDay()]}`;
}

/** Epoch-ms for a Y/M/D at local noon (robust to DST shifts). */
function msFor(y: number, m: number, d: number): number {
  return new Date(y, m - 1, d, 12, 0, 0).getTime();
}

/** Build a single MediaItemDto with the given capture time. */
function item(mediaId: string, dateModifiedMs: number): MediaItemDto {
  return { mediaId, uri: `content://media/${mediaId}`, dateModifiedMs, width: 100, height: 100, mimeType: 'image/jpeg', displayName: null, filePath: null };
}

// ---- Mocks ----

// Newest-first items spanning three distinct days, each with a different count.
const D_DAY1 = msFor(2026, 7, 19); // newest — 1 photo
const D_DAY2 = msFor(2026, 7, 18); // — 2 photos
const D_DAY3 = msFor(2026, 6, 30); // oldest — 3 photos
const mockItems: MediaItemDto[] = [
  item('d1-a', D_DAY1),
  item('d2-a', D_DAY2),
  item('d2-b', D_DAY2),
  item('d3-a', D_DAY3),
  item('d3-b', D_DAY3),
  item('d3-c', D_DAY3),
];

const mockLoadAll = vi.fn().mockResolvedValue(undefined);
let mockCursor: string | null = null;

vi.mock('../../hooks/useGalleryPager', () => ({
  useGalleryPager: () => ({
    items: mockItems,
    cursor: mockCursor,
    totalCount: mockItems.length,
    isLoading: false,
    error: null,
    loadNextPage: vi.fn(),
    reload: vi.fn().mockResolvedValue(undefined),
    loadAll: mockLoadAll,
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
  useImagePreviewOpener: () => vi.fn(),
}));

vi.mock('../../hooks/useAndroidAutoOpenLatestPhoto', () => ({
  useAndroidAutoOpenLatestPhoto: () => {},
}));

vi.mock('../../services/gallery-media-v2', () => ({
  isGalleryV2Available: () => true,
  invalidateMediaIds: vi.fn().mockResolvedValue(undefined),
}));

vi.mock('../../stores/configStore', () => ({
  useConfigStore: () => ({ activeTab: 'gallery' }),
  useDraftConfig: () => null,
}));

vi.mock('../../stores/permissionStore', () => ({
  usePermissionStore: () => ({ requestStoragePermission: vi.fn(), startPolling: vi.fn() }),
}));

// ---- Tests ----

describe('GalleryCard date-jump', () => {
  const { getContainer, getRoot } = setupReactRoot();
  let resizeMock: ReturnType<typeof createMockRectObserver>;
  let originalResizeObserver: typeof ResizeObserver;

  beforeEach(() => {
    resizeMock = createMockRectObserver();
    originalResizeObserver = window.ResizeObserver;
    window.ResizeObserver = resizeMock.MockResizeObserver as unknown as typeof ResizeObserver;
    mockLoadAll.mockClear();
    mockCursor = null;
  });

  afterEach(() => {
    window.ResizeObserver = originalResizeObserver;
  });

  it('title shows the first visible photo date instead of a count', async () => {
    await act(async () => {
      getRoot().render(<GalleryCard />);
      await flush();
    });

    const title = getContainer().querySelector('[data-testid="gallery-date-title"]');
    expect(title).toBeTruthy();
    // Newest photo is 2026-7-19 (周日) — the gallery is sorted dateDesc.
    expect(title!.textContent).toContain(titleFor(D_DAY1));
    expect(title!.textContent).not.toContain('图库 (');
  });

  it('opens the date dialog with all unique days and their counts', async () => {
    await act(async () => {
      getRoot().render(<GalleryCard />);
      await flush();
    });

    const title = getContainer().querySelector('[data-testid="gallery-date-title"]') as HTMLButtonElement;
    await act(async () => {
      title.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    const dialog = getContainer().querySelector('[data-testid="date-jump-dialog"]');
    expect(dialog).toBeTruthy();

    // Three unique days, newest-first.
    const rows = Array.from(dialog!.querySelectorAll('[data-date-key]'));
    expect(rows.length).toBe(3);
    expect(rows[0].getAttribute('data-date-key')).toBe('2026-7-19');
    expect(rows[1].getAttribute('data-date-key')).toBe('2026-7-18');
    expect(rows[2].getAttribute('data-date-key')).toBe('2026-6-30');

    // Each row shows its day's photo count.
    expect(rows[0].textContent).toContain('1 张');
    expect(rows[1].textContent).toContain('2 张');
    expect(rows[2].textContent).toContain('3 张');

    // The currently displayed day is marked active.
    const active = dialog!.querySelector('button.bg-blue-50');
    expect(active).toBeTruthy();
    expect(active!.textContent).toContain(titleFor(D_DAY1));
  });

  it('loads all remaining pages before opening when more data exists', async () => {
    mockCursor = 'more-data';

    await act(async () => {
      getRoot().render(<GalleryCard />);
      await flush();
    });

    const title = getContainer().querySelector('[data-testid="gallery-date-title"]') as HTMLButtonElement;
    await act(async () => {
      title.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
      await flush();
    });

    expect(mockLoadAll).toHaveBeenCalledTimes(1);
  });

  it('scrolls the grid to the selected day on pick', async () => {
    await act(async () => {
      getRoot().render(<GalleryCard />);
      await flush();
    });

    // Open the dialog.
    const title = getContainer().querySelector('[data-testid="gallery-date-title"]') as HTMLButtonElement;
    await act(async () => {
      title.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    // jsdom lacks Element.scrollTo; stub it so VirtualGalleryGrid.scrollToIndex
    // (which calls container.scrollTo({ top })) can be observed.
    const gridContainer = getContainer().querySelector('[data-testid="virtual-grid-container"]') as HTMLDivElement;
    expect(gridContainer).toBeTruthy();
    let capturedTop = -1;
    gridContainer!.scrollTo = ((opts: { top?: number }) => { capturedTop = opts?.top ?? -1; }) as Element['scrollTo'];

    // Pick the oldest day (2026-6-30 → first item of that day is at index 3).
    const oldestRow = getContainer().querySelector('[data-date-key="2026-6-30"]') as HTMLButtonElement;
    expect(oldestRow).toBeTruthy();
    await act(async () => {
      oldestRow.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    // index 3 → row floor(3/3) = 1 → scrollTop = 1 * 120 = 120
    expect(capturedTop).toBe(120);

    // The highlight request is deferred to the next animation frame (real
    // timers here, so flush() won't drain it). Wait one frame, then let the
    // grid's latch effect arm the pulse once the target cell mounts.
    await act(async () => {
      await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
      await flush();
    });

    // The landed-on cell pulses with a highlight overlay (visible feedback
    // even when the scroll is a no-op, e.g. single-page galleries).
    const overlay = getContainer().querySelector('[data-testid="highlight-overlay"]');
    expect(overlay).toBeTruthy();

    // Dialog closes after selection.
    expect(getContainer().querySelector('[data-testid="date-jump-dialog"]')).toBeNull();
  });

  it('clears the highlight overlay shortly after a jump', async () => {
    vi.useFakeTimers();
    try {
      await act(async () => {
        getRoot().render(<GalleryCard />);
        await flush();
      });

      const title = getContainer().querySelector('[data-testid="gallery-date-title"]') as HTMLButtonElement;
      await act(async () => {
        title.dispatchEvent(new MouseEvent('click', { bubbles: true }));
        await flush();
      });
      // jsdom lacks Element.scrollTo; stub it before the jump triggers one.
      const gridContainer = getContainer().querySelector('[data-testid="virtual-grid-container"]') as HTMLDivElement;
      gridContainer!.scrollTo = (() => {}) as Element['scrollTo'];
      const row = getContainer().querySelector('[data-date-key="2026-6-30"]') as HTMLButtonElement;

      await act(async () => {
        row.dispatchEvent(new MouseEvent('click', { bubbles: true }));
        // handleDateJump defers setHighlightMediaId via requestAnimationFrame;
        // flush that, then the grid's latch effect arms the pulse + clear timer.
        await vi.runAllTimersAsync();
      });

      // Highlight is armed once the target cell is in the render window.
      expect(getContainer().querySelector('[data-testid="highlight-overlay"]')).toBeTruthy();

      // …and is removed once the pulse animation window elapses.
      await act(async () => {
        vi.advanceTimersByTime(900);
        await flush();
      });
      expect(getContainer().querySelector('[data-testid="highlight-overlay"]')).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });
});
