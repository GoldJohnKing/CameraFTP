/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { act } from 'react';
import { beforeEach, afterEach, describe, expect, it, vi } from 'vitest';
import { GalleryCard } from '../GalleryCard';
import type { MediaItemDto } from '../../types';
import { makeItems } from '../../test-utils/media-factory';
import { setupReactRoot } from '../../test-utils/react-root';

// ---- Mocks ----

const mockItems = makeItems(300);
let mockLoadNextPage = vi.fn();
let mockReload = vi.fn();
let mockRemoveItems = vi.fn();
let mockIsLoading = false;
let mockError: string | null = null;

vi.mock('../../hooks/useGalleryPager', () => ({
  useGalleryPager: () => ({
    items: mockItems,
    cursor: null,
    revisionToken: '',
    isLoading: mockIsLoading,
    error: mockError,
    loadNextPage: mockLoadNextPage,
    reload: mockReload,
    removeItems: mockRemoveItems,
  }),
}));

const mockUpdateViewport = vi.fn();
const mockRemoveThumbs = vi.fn();
const mockCleanup = vi.fn();
const mockRegisterMedia = vi.fn();

vi.mock('../../hooks/useThumbnailScheduler', () => ({
  useThumbnailScheduler: () => ({
    thumbnails: new Map<string, string>(),
    loadingThumbs: new Set<string>(),
    updateViewport: mockUpdateViewport,
    removeThumbs: mockRemoveThumbs,
    cleanup: mockCleanup,
    registerMedia: mockRegisterMedia,
  }),
}));

let mockGallerySelectionOverrides: Record<string, unknown> = {};
let capturedOnDeleteApplied: ((ids: string[]) => Promise<void>) | null = null;

vi.mock('../../hooks/useGallerySelection', () => ({
  useGallerySelection: (args: { onDeleteApplied?: (ids: string[]) => Promise<void> }) => {
    capturedOnDeleteApplied = args?.onDeleteApplied ?? null;
    return {
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
      ...mockGallerySelectionOverrides,
    };
  },
}));

vi.mock('../../hooks/useImagePreviewOpener', () => ({
  useImagePreviewOpener: () => vi.fn(),
}));

vi.mock('../../services/gallery-media-v2', () => ({
  isGalleryV2Available: () => true,
  invalidateMediaIds: vi.fn().mockResolvedValue(undefined),
}));

vi.mock('../../stores/configStore', () => ({
  useConfigStore: () => ({ activeTab: 'gallery' }),
}));

import { flush } from '../../test-utils/flush';
import { createMockRectObserver } from '../../test-utils/mock-resize-observer';

// ---- Tests ----

describe('GalleryCard (virtualized)', () => {
  const { getContainer, getRoot } = setupReactRoot();
  let resizeMock: ReturnType<typeof createMockRectObserver>;
  let originalResizeObserver: typeof ResizeObserver;

  beforeEach(() => {
    resizeMock = createMockRectObserver();
    originalResizeObserver = window.ResizeObserver;
    window.ResizeObserver = resizeMock.MockResizeObserver as unknown as typeof ResizeObserver;

    mockLoadNextPage.mockClear();
    mockReload.mockClear();
    mockRemoveItems.mockClear();
    mockUpdateViewport.mockClear();
    mockRemoveThumbs.mockClear();
    mockCleanup.mockClear();
    mockRegisterMedia.mockClear();
    mockIsLoading = false;
    mockError = null;
    mockGallerySelectionOverrides = {};
    capturedOnDeleteApplied = null;
  });

  afterEach(() => {
    window.ResizeObserver = originalResizeObserver;
  });

  it('renders with virtualized grid and does not mount all items', async () => {
    await act(async () => {
      getRoot().render(<GalleryCard />);
      await flush();
    });

    // Simulate container height = 360px (3 visible rows at 120px each)
    const gridContainer = getContainer().querySelector('[data-testid="virtual-grid-container"]');
    expect(gridContainer).toBeTruthy();

    if (gridContainer) {
      act(() => {
        resizeMock.triggerResize(gridContainer, 360);
      });
    }

    await flush();

    // With 300 items but only visible+overscan rendered, we should see far fewer cells
    const renderedCells = getContainer().querySelectorAll('[data-media-id]');
    expect(renderedCells.length).toBeGreaterThan(0);
    expect(renderedCells.length).toBeLessThan(mockItems.length);
    // Virtual grid renders only visible rows + overscan, not all 300 items
    expect(renderedCells.length).toBeLessThan(30);
  });

  it('calls loadNextPage on mount', async () => {
    await act(async () => {
      getRoot().render(<GalleryCard />);
      await flush();
    });

    expect(mockLoadNextPage).toHaveBeenCalledTimes(1);
  });

  it('calls registerMedia when items are available', async () => {
    await act(async () => {
      getRoot().render(<GalleryCard />);
      await flush();
    });

    expect(mockRegisterMedia).toHaveBeenCalledWith(mockItems);
  });

  it('shows error state when pager has error', async () => {
    mockError = 'Network failure';

    await act(async () => {
      getRoot().render(<GalleryCard />);
      await flush();
    });

    expect(getContainer().textContent).toContain('Network failure');
    expect(getContainer().querySelector('[data-testid="virtual-grid-container"]')).toBeNull();
  });

  it('shows empty state when no items and not loading', async () => {
    const savedLength = mockItems.length;
    (mockItems as MediaItemDto[]).length = 0;

    await act(async () => {
      getRoot().render(<GalleryCard />);
      await flush();
    });

    expect(getContainer().textContent).toContain('暂无图片');

    // Restore
    (mockItems as MediaItemDto[]).length = savedLength;
  });

  it('reports viewport range changes to scheduler', async () => {
    await act(async () => {
      getRoot().render(<GalleryCard />);
    });
    await flush();
    await flush();

    const gridContainer = getContainer().querySelector('[data-testid="virtual-grid-container"]');

    if (gridContainer) {
      act(() => {
        resizeMock.triggerResize(gridContainer, 360);
      });
      await flush();
      await flush();
    }

    // Verify the grid rendered and viewport updates were reported
    expect(gridContainer).not.toBeNull();
    expect(mockUpdateViewport).toHaveBeenCalled();
    const call = mockUpdateViewport.mock.calls[mockUpdateViewport.mock.calls.length - 1];
    expect(call[0].length).toBeGreaterThan(0);
    expect(Array.isArray(call[1])).toBe(true);
  });

  it('calls reload and cleanup on refresh', async () => {
    await act(async () => {
      getRoot().render(<GalleryCard />);
      await flush();
    });

    const refreshButton = getContainer().querySelector('button');
    expect(refreshButton).toBeTruthy();

    await act(async () => {
      refreshButton!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    expect(mockReload).toHaveBeenCalledTimes(1);
    expect(mockCleanup).toHaveBeenCalledTimes(1);
  });

  it('calls removeItems and removeThumbs on delete', async () => {
    const deletedIds = ['id1'];
    mockGallerySelectionOverrides = {
      isSelectionMode: true,
      selectedIds: new Set(deletedIds),
      showMenu: true,
      handleDelete: vi.fn(async () => {
        if (capturedOnDeleteApplied) {
          await capturedOnDeleteApplied(deletedIds);
        }
      }),
    };

    await act(async () => {
      getRoot().render(<GalleryCard />);
      await flush();
    });

    const deleteButton = Array.from(getContainer().querySelectorAll('button')).find((btn) =>
      btn.textContent?.includes('删除'),
    );
    expect(deleteButton).toBeTruthy();

    await act(async () => {
      deleteButton!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    expect(mockRemoveItems).toHaveBeenCalledWith(deletedIds);
    expect(mockRemoveThumbs).toHaveBeenCalledWith(deletedIds);
  });
});
