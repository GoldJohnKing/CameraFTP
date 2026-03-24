/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { VirtualGalleryGrid } from '../VirtualGalleryGrid';
import type { MediaItemDto } from '../../types/gallery-v2';

function makeItems(count: number): MediaItemDto[] {
  return Array.from({ length: count }, (_, i) => ({
    mediaId: `media-${i}`,
    uri: `content://media/${i}`,
    dateModifiedMs: 1000 + i,
    width: 100,
    height: 100,
    mimeType: 'image/jpeg',
    displayName: null,
  }));
}

function createMockRectObserver() {
  const callbacks: Map<Element, ResizeObserverCallback> = new Map();

  class MockResizeObserver {
    private _cb: ResizeObserverCallback;
    private _el: Element | null = null;

    constructor(cb: ResizeObserverCallback) {
      this._cb = cb;
    }

    observe(el: Element) {
      this._el = el;
      callbacks.set(el, this._cb);
    }

    unobserve(el: Element) {
      callbacks.delete(el);
    }

    disconnect() {
      if (this._el) callbacks.delete(this._el);
    }
  }

  const triggerResize = (el: Element, height: number) => {
    const cb = callbacks.get(el);
    if (cb) {
      cb(
        [{ contentRect: { height } } as ResizeObserverEntry],
        {} as ResizeObserver
      );
    }
  };

  return { MockResizeObserver, triggerResize };
}

async function flush(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}

describe('VirtualGalleryGrid', () => {
  let container: HTMLDivElement;
  let root: Root;
  let resizeMock: ReturnType<typeof createMockRectObserver>;
  let originalResizeObserver: typeof ResizeObserver;

  beforeEach(() => {
    vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);

    resizeMock = createMockRectObserver();
    originalResizeObserver = window.ResizeObserver;
    window.ResizeObserver = resizeMock.MockResizeObserver as unknown as typeof ResizeObserver;
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    window.ResizeObserver = originalResizeObserver;
    vi.unstubAllGlobals();
  });

  it('renders only visible + overscan cells, not all items', async () => {
    const items = makeItems(300); // 100 rows at 3 columns
    const onRangeChange = vi.fn();

    await act(async () => {
      root.render(
        <VirtualGalleryGrid
          items={items}
          thumbnails={new Map()}
          loadingThumbs={new Set()}
          onItemClick={vi.fn()}
          onRangeChange={onRangeChange}
          rowHeight={120}
          overscanRows={3}
        />
      );
      await flush();
    });

    // Simulate container height = 360px (3 visible rows)
    const gridContainer = container.querySelector('[data-testid="virtual-grid-container"]');
    expect(gridContainer).toBeTruthy();

    if (gridContainer) {
      act(() => {
        resizeMock.triggerResize(gridContainer, 360);
      });
    }

    await flush();

    // With 360px height and 120px rowHeight: visibleEndRow = floor(360/120) = 3 (rows 0-3)
    // With 3 overscan rows below: endRow = min(99, 3+3) = 6
    // At scrollTop=0, startRow = max(0, 0-3) = 0, so rows 0-6 = 7 rows = 21 cells
    const renderedCells = container.querySelectorAll('[data-media-id]');
    expect(renderedCells.length).toBeLessThan(items.length);
    expect(renderedCells.length).toBeGreaterThan(0);
    expect(renderedCells.length).toBe(21);
  });

  it('reports visible range changes on scroll', async () => {
    const items = makeItems(90); // 30 rows
    const onRangeChange = vi.fn();

    await act(async () => {
      root.render(
        <VirtualGalleryGrid
          items={items}
          thumbnails={new Map()}
          loadingThumbs={new Set()}
          onItemClick={vi.fn()}
          onRangeChange={onRangeChange}
          rowHeight={120}
          overscanRows={2}
        />
      );
      await flush();
    });

    const gridContainer = container.querySelector('[data-testid="virtual-grid-container"]');
    expect(gridContainer).toBeTruthy();

    // Simulate container height = 360px (3 visible rows)
    if (gridContainer) {
      act(() => {
        resizeMock.triggerResize(gridContainer, 360);
      });
    }
    await flush();

    // Initial range report
    expect(onRangeChange).toHaveBeenCalled();
    const initialCall = onRangeChange.mock.calls[onRangeChange.mock.calls.length - 1];
    const [initialVisible] = initialCall;
    expect(initialVisible).toContain('media-0');
    expect(initialVisible).toContain('media-8'); // row 0-2 = items 0-8

    // Simulate scroll to row 10 (scrollTop = 1200)
    if (gridContainer) {
      Object.defineProperty(gridContainer, 'scrollTop', {
        value: 1200,
        writable: true,
        configurable: true,
      });
      act(() => {
        gridContainer.dispatchEvent(new Event('scroll'));
      });
    }
    await flush();

    // After scroll, visible range should have shifted
    const scrollCall = onRangeChange.mock.calls[onRangeChange.mock.calls.length - 1];
    const [scrollVisible] = scrollCall;
    // At scrollTop=1200, visibleStartRow = 10, visibleEndRow = 12
    // Items 30-35 (rows 10-12)
    expect(scrollVisible).toContain('media-30');
    expect(scrollVisible).not.toContain('media-0');
  });

  it('shows placeholder for items without thumbnail', async () => {
    const items = makeItems(3);

    await act(async () => {
      root.render(
        <VirtualGalleryGrid
          items={items}
          thumbnails={new Map()}
          loadingThumbs={new Set()}
          onItemClick={vi.fn()}
        />
      );
      await flush();
    });

    const gridContainer = container.querySelector('[data-testid="virtual-grid-container"]');
    if (gridContainer) {
      act(() => {
        resizeMock.triggerResize(gridContainer, 360);
      });
    }
    await flush();

    const pulseElements = container.querySelectorAll('.animate-pulse');
    expect(pulseElements.length).toBe(3);
  });

  it('shows spinner for items with loading thumbnail', async () => {
    const items = makeItems(3);
    const loadingThumbs = new Set(['media-0', 'media-1', 'media-2']);

    await act(async () => {
      root.render(
        <VirtualGalleryGrid
          items={items}
          thumbnails={new Map()}
          loadingThumbs={loadingThumbs}
          onItemClick={vi.fn()}
        />
      );
      await flush();
    });

    const gridContainer = container.querySelector('[data-testid="virtual-grid-container"]');
    if (gridContainer) {
      act(() => {
        resizeMock.triggerResize(gridContainer, 360);
      });
    }
    await flush();

    const spinners = container.querySelectorAll('.animate-spin');
    expect(spinners.length).toBe(3);
  });

  it('shows img for items with loaded thumbnail', async () => {
    const items = makeItems(3);
    const thumbnails = new Map([
      ['media-0', 'blob://thumb-0'],
      ['media-1', 'blob://thumb-1'],
      ['media-2', 'blob://thumb-2'],
    ]);

    await act(async () => {
      root.render(
        <VirtualGalleryGrid
          items={items}
          thumbnails={thumbnails}
          loadingThumbs={new Set()}
          onItemClick={vi.fn()}
        />
      );
      await flush();
    });

    const gridContainer = container.querySelector('[data-testid="virtual-grid-container"]');
    if (gridContainer) {
      act(() => {
        resizeMock.triggerResize(gridContainer, 360);
      });
    }
    await flush();

    const images = container.querySelectorAll('img');
    expect(images.length).toBe(3);
    expect(images[0].getAttribute('src')).toBe('blob://thumb-0');
  });

  it('calls onItemClick when a cell is clicked', async () => {
    const items = makeItems(3);
    const onItemClick = vi.fn();

    await act(async () => {
      root.render(
        <VirtualGalleryGrid
          items={items}
          thumbnails={new Map()}
          loadingThumbs={new Set()}
          onItemClick={onItemClick}
        />
      );
      await flush();
    });

    const gridContainer = container.querySelector('[data-testid="virtual-grid-container"]');
    if (gridContainer) {
      act(() => {
        resizeMock.triggerResize(gridContainer, 360);
      });
    }
    await flush();

    const firstCell = container.querySelector('[data-media-id="media-0"]');
    expect(firstCell).toBeTruthy();

    act(() => {
      firstCell!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    expect(onItemClick).toHaveBeenCalledTimes(1);
    expect(onItemClick).toHaveBeenCalledWith(items[0]);
  });
});
