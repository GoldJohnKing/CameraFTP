/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { act } from 'react';
import { beforeEach, afterEach, describe, expect, it, vi } from 'vitest';
import { VirtualGalleryGrid, type VirtualGalleryGridHandle } from '../VirtualGalleryGrid';
import { flush } from '../../test-utils/flush';
import { makeItems } from '../../test-utils/media-factory';
import { createMockRectObserver } from '../../test-utils/mock-resize-observer';
import { setupReactRoot } from '../../test-utils/react-root';

/** inner grid 元素的“真实布局”计算样式（380px 宽 + 实际 padding/gap）。 */
const INNER_GRID_STYLE = {
  width: '380px',
  paddingLeft: '2px',
  paddingRight: '2px',
  paddingTop: '4px',
  paddingBottom: '6px',
  columnGap: '6px',
  rowGap: '6px',
} as const;

/**
 * 仅替换对 virtual-grid-inner 元素的 getComputedStyle（其余元素走 jsdom 原
 * 实现），用于模拟浏览器完成布局后 measureGridMetrics 读到的真实值。
 * 返回还原函数（务必在 finally 中调用，避免污染其它用例）。
 */
function stubInnerGridComputedStyle(): () => void {
  const original = window.getComputedStyle;
  window.getComputedStyle = ((elt: Element, pseudoElt?: string | null) => {
    if ((elt as HTMLElement).dataset?.testid === 'virtual-grid-inner') {
      return { ...INNER_GRID_STYLE } as unknown as CSSStyleDeclaration;
    }
    return original(elt, pseudoElt ?? undefined);
  }) as typeof window.getComputedStyle;
  return () => {
    window.getComputedStyle = original;
  };
}

describe('VirtualGalleryGrid', () => {
  const { getContainer, getRoot } = setupReactRoot();
  let resizeMock: ReturnType<typeof createMockRectObserver>;
  let originalResizeObserver: typeof ResizeObserver;

  beforeEach(() => {
    resizeMock = createMockRectObserver();
    originalResizeObserver = window.ResizeObserver;
    window.ResizeObserver = resizeMock.MockResizeObserver as unknown as typeof ResizeObserver;
  });

  afterEach(() => {
    window.ResizeObserver = originalResizeObserver;
  });

  it('renders only visible + overscan cells, not all items', async () => {
    const CONTAINER_HEIGHT = 360;
    // jsdom 无法解析网格宽度 → 回退 pitch = DEFAULT_GRID_METRICS.pitch = 120；
    // COLUMNS=3, OVERSCAN_ROWS=3（组件内部常量）
    // visibleRows = ceil(360/120) = 3, overscan=3, renderedRows = 3+3+1 = 7
    const COLUMNS = 3;
    const expectedCellCount = 7 * COLUMNS; // 21
    const items = makeItems(300); // 100 rows at 3 columns
    const onRangeChange = vi.fn();

    await act(async () => {
      getRoot().render(
        <VirtualGalleryGrid
          items={items}
          thumbnails={new Map()}
          loadingThumbs={new Set()}
          onItemClick={vi.fn()}
          onRangeChange={onRangeChange}
        />
      );
      await flush();
    });

    // Simulate container height = 360px (3 visible rows)
    const gridContainer = getContainer().querySelector('[data-testid="virtual-grid-container"]');
    expect(gridContainer).toBeTruthy();

    if (gridContainer) {
      act(() => {
        resizeMock.triggerResize(gridContainer, CONTAINER_HEIGHT);
      });
    }

    await flush();

    // With 360px height and jsdom fallback pitch 120 (DEFAULT_GRID_METRICS):
    // visibleEndRow = floor(360/120) = 3 (rows 0-3)
    // With 3 overscan rows below: endRow = min(99, 3+3) = 6
    // At scrollTop=0, startRow = max(0, 0-3) = 0, so rows 0-6 = 7 rows = 21 cells
    const renderedCells = getContainer().querySelectorAll('[data-media-id]');
    expect(renderedCells.length).toBeLessThan(items.length);
    expect(renderedCells.length).toBeGreaterThan(0);
    expect(renderedCells.length).toBe(expectedCellCount);
  });

  it('reports visible range changes on scroll', async () => {
    const items = makeItems(90); // 30 rows
    const onRangeChange = vi.fn();

    await act(async () => {
      getRoot().render(
        <VirtualGalleryGrid
          items={items}
          thumbnails={new Map()}
          loadingThumbs={new Set()}
          onItemClick={vi.fn()}
          onRangeChange={onRangeChange}
        />
      );
      await flush();
    });

    const gridContainer = getContainer().querySelector('[data-testid="virtual-grid-container"]');
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
      getRoot().render(
        <VirtualGalleryGrid
          items={items}
          thumbnails={new Map()}
          loadingThumbs={new Set()}
          onItemClick={vi.fn()}
        />
      );
      await flush();
    });

    const gridContainer = getContainer().querySelector('[data-testid="virtual-grid-container"]');
    if (gridContainer) {
      act(() => {
        resizeMock.triggerResize(gridContainer, 360);
      });
    }
    await flush();

    const pulseElements = getContainer().querySelectorAll('.animate-pulse');
    expect(pulseElements.length).toBe(3);
  });

  it('shows spinner for items with loading thumbnail', async () => {
    const items = makeItems(3);
    const loadingThumbs = new Set(['media-0', 'media-1', 'media-2']);

    await act(async () => {
      getRoot().render(
        <VirtualGalleryGrid
          items={items}
          thumbnails={new Map()}
          loadingThumbs={loadingThumbs}
          onItemClick={vi.fn()}
        />
      );
      await flush();
    });

    const gridContainer = getContainer().querySelector('[data-testid="virtual-grid-container"]');
    if (gridContainer) {
      act(() => {
        resizeMock.triggerResize(gridContainer, 360);
      });
    }
    await flush();

    const spinners = getContainer().querySelectorAll('.animate-spin');
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
      getRoot().render(
        <VirtualGalleryGrid
          items={items}
          thumbnails={thumbnails}
          loadingThumbs={new Set()}
          onItemClick={vi.fn()}
        />
      );
      await flush();
    });

    const gridContainer = getContainer().querySelector('[data-testid="virtual-grid-container"]');
    if (gridContainer) {
      act(() => {
        resizeMock.triggerResize(gridContainer, 360);
      });
    }
    await flush();

    const images = getContainer().querySelectorAll('img');
    expect(images.length).toBe(3);
    expect(images[0].getAttribute('src')).toBe('blob://thumb-0');
  });

  it('calls onItemClick when a cell is clicked', async () => {
    const items = makeItems(3);
    const onItemClick = vi.fn();

    await act(async () => {
      getRoot().render(
        <VirtualGalleryGrid
          items={items}
          thumbnails={new Map()}
          loadingThumbs={new Set()}
          onItemClick={onItemClick}
        />
      );
      await flush();
    });

    const gridContainer = getContainer().querySelector('[data-testid="virtual-grid-container"]');
    if (gridContainer) {
      act(() => {
        resizeMock.triggerResize(gridContainer, 360);
      });
    }
    await flush();

    const firstCell = getContainer().querySelector('[data-media-id="media-0"]');
    expect(firstCell).toBeTruthy();

    act(() => {
      firstCell!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });

    expect(onItemClick).toHaveBeenCalledTimes(1);
    expect(onItemClick).toHaveBeenCalledWith(items[0]);
  });

  it('arms the highlight overlay only after the target scrolls into view', async () => {
    // Regression: a highlight requested while the target is outside the render
    // window (e.g. a multi-page date-jump) must still pulse once the scroll
    // settles and the cell mounts — not be silently dropped.
    const CONTAINER_HEIGHT = 360; // 3 visible rows
    const items = makeItems(300); // 100 rows; target media-90 lives on row 30
    const TARGET_ID = 'media-90';

    await act(async () => {
      getRoot().render(
        <VirtualGalleryGrid
          items={items}
          thumbnails={new Map()}
          loadingThumbs={new Set()}
          onItemClick={vi.fn()}
          highlightMediaId={TARGET_ID}
        />
      );
      await flush();
    });

    const gridContainer = getContainer().querySelector('[data-testid="virtual-grid-container"]');
    expect(gridContainer).toBeTruthy();
    if (gridContainer) {
      act(() => {
        resizeMock.triggerResize(gridContainer, CONTAINER_HEIGHT);
      });
    }
    await flush();

    // Target is far below the initial window — no overlay yet.
    expect(getContainer().querySelector('[data-testid="highlight-overlay"]')).toBeNull();

    // Simulate the scroll the jump would trigger: scrollTop to row 30 = 3600.
    if (gridContainer) {
      Object.defineProperty(gridContainer, 'scrollTop', {
        value: 30 * 120,
        writable: true,
        configurable: true,
      });
      act(() => {
        gridContainer.dispatchEvent(new Event('scroll'));
      });
    }
    await flush();

    // Now the target is in the render window; the latch arms the pulse.
    expect(getContainer().querySelector('[data-testid="highlight-overlay"]')).toBeTruthy();
  });

  it('does not replay the highlight when items reload (refresh)', async () => {
    // Regression: highlightMediaId is not cleared after a pulse. A gallery
    // refresh rebuilds the items array (new identity) → visibleItems changes →
    // the latch effect used to re-run and re-arm the pulse even though no new
    // jump happened. The pulse must fire exactly once per request.
    vi.useFakeTimers();
    try {
      const CONTAINER_HEIGHT = 360; // 3 visible rows
      const TARGET_ID = 'media-0';  // lives in the first render window

      await act(async () => {
        getRoot().render(
          <VirtualGalleryGrid
            items={makeItems(9)}
            thumbnails={new Map()}
            loadingThumbs={new Set()}
            onItemClick={vi.fn()}
            highlightMediaId={TARGET_ID}
          />
        );
        await flush();
      });

      const gridContainer = getContainer().querySelector('[data-testid="virtual-grid-container"]');
      if (gridContainer) {
        act(() => {
          resizeMock.triggerResize(gridContainer, CONTAINER_HEIGHT);
        });
      }
      await flush();

      // Pulse arms on mount (target is in the first render window).
      expect(getContainer().querySelector('[data-testid="highlight-overlay"]')).toBeTruthy();

      // …and clears once the pulse window elapses.
      await act(async () => {
        vi.advanceTimersByTime(900);
        await flush();
      });
      expect(getContainer().querySelector('[data-testid="highlight-overlay"]')).toBeNull();

      // Simulate a gallery refresh: pager.reload() returns a fresh items array
      // (same content, new identity) while highlightMediaId is still set.
      await act(async () => {
        getRoot().render(
          <VirtualGalleryGrid
            items={makeItems(9)}
            thumbnails={new Map()}
            loadingThumbs={new Set()}
            onItemClick={vi.fn()}
            highlightMediaId={TARGET_ID}
          />
        );
        await flush();
      });

      // Regression guard: the pulse must NOT replay just because items reloaded.
      expect(getContainer().querySelector('[data-testid="highlight-overlay"]')).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });

  it('cancels an in-flight pulse when items reload (refresh during the pulse)', async () => {
    // Regression: a gallery refresh empties items (pager.reload sets []) then
    // refills them. If the refresh happens while the pulse is still playing,
    // the armed activeHighlight used to survive the empty→refill and re-render
    // the overlay on the remounted cell — looking like refresh re-triggered the
    // animation. The grid must cancel the in-flight pulse when the dataset is
    // emptied. (We deliberately do NOT advance the 850ms clear timer, so the
    // pulse is still in-flight when the reload happens.)
    vi.useFakeTimers();
    try {
      const CONTAINER_HEIGHT = 360; // 3 visible rows
      const TARGET_ID = 'media-0';  // lives in the first render window

      await act(async () => {
        getRoot().render(
          <VirtualGalleryGrid
            items={makeItems(9)}
            thumbnails={new Map()}
            loadingThumbs={new Set()}
            onItemClick={vi.fn()}
            highlightMediaId={TARGET_ID}
          />
        );
        await flush();
      });

      const gridContainer = getContainer().querySelector('[data-testid="virtual-grid-container"]');
      if (gridContainer) {
        act(() => {
          resizeMock.triggerResize(gridContainer, CONTAINER_HEIGHT);
        });
      }
      await flush();

      // Pulse is armed and still in-flight (timer not advanced).
      expect(getContainer().querySelector('[data-testid="highlight-overlay"]')).toBeTruthy();

      // Simulate the start of a refresh: pager.reload() empties items.
      await act(async () => {
        getRoot().render(
          <VirtualGalleryGrid
            items={[]}
            thumbnails={new Map()}
            loadingThumbs={new Set()}
            onItemClick={vi.fn()}
            highlightMediaId={TARGET_ID}
          />
        );
        await flush();
      });

      // Simulate the refresh completing: items refill (same target present).
      await act(async () => {
        getRoot().render(
          <VirtualGalleryGrid
            items={makeItems(9)}
            thumbnails={new Map()}
            loadingThumbs={new Set()}
            onItemClick={vi.fn()}
            highlightMediaId={TARGET_ID}
          />
        );
        await flush();
      });

      // The in-flight pulse must NOT reappear after the reload.
      expect(getContainer().querySelector('[data-testid="highlight-overlay"]')).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });

  it('does not re-report the range when only callback identities change', async () => {
    const items = makeItems(90); // 30 rows — same array instance for both renders
    const onRangeChange1 = vi.fn();

    await act(async () => {
      getRoot().render(
        <VirtualGalleryGrid
          items={items}
          thumbnails={new Map()}
          loadingThumbs={new Set()}
          onItemClick={vi.fn()}
          onRangeChange={onRangeChange1}
        />
      );
      await flush();
    });

    const gridContainer = getContainer().querySelector('[data-testid="virtual-grid-container"]');
    expect(gridContainer).toBeTruthy();
    if (gridContainer) {
      act(() => {
        resizeMock.triggerResize(gridContainer, 360);
      });
    }
    await flush();

    const callsAfterMount = onRangeChange1.mock.calls.length;
    expect(callsAfterMount).toBeGreaterThanOrEqual(1);

    // Same items/scroll, brand-new callback identities — exactly what happens
    // on every GalleryCard render while thumbnails stream in.
    const onRangeChange2 = vi.fn();
    await act(async () => {
      getRoot().render(
        <VirtualGalleryGrid
          items={items}
          thumbnails={new Map()}
          loadingThumbs={new Set()}
          onItemClick={vi.fn()}
          onRangeChange={onRangeChange2}
          onNearEnd={vi.fn()}
        />
      );
      await flush();
    });

    expect(onRangeChange2).not.toHaveBeenCalled();
    expect(onRangeChange1.mock.calls.length).toBe(callsAfterMount);
  });

  it('retries onNearEnd when the effect re-runs on callback identity change (same items/range)', async () => {
    // Regression: the near-end check used to sit AFTER the "same items+range"
    // short-circuit. When the parent re-rendered with fresh callback
    // identities while the viewport was near the end, the effect returned
    // early and the pagination trigger was silently dropped.
    const CONTAINER_HEIGHT = 360; // 3 visible rows at fallback pitch 120
    const items = makeItems(27); // 9 rows → rowsRemaining = 9 - 3 - 1 = 5 ≤ threshold
    const onNearEnd1 = vi.fn();

    await act(async () => {
      getRoot().render(
        <VirtualGalleryGrid
          items={items}
          thumbnails={new Map()}
          loadingThumbs={new Set()}
          onItemClick={vi.fn()}
          onRangeChange={vi.fn()}
          onNearEnd={onNearEnd1}
        />
      );
      await flush();
    });

    const gridContainer = getContainer().querySelector('[data-testid="virtual-grid-container"]');
    expect(gridContainer).toBeTruthy();
    if (gridContainer) {
      act(() => {
        resizeMock.triggerResize(gridContainer, CONTAINER_HEIGHT);
      });
    }
    await flush();

    // Near-end fired with the initial range report.
    expect(onNearEnd1).toHaveBeenCalledTimes(1);

    // Same items/same visible range, brand-new callback identities — exactly
    // what a GalleryCard re-render produces while thumbnails stream in. The
    // range report is deduped, but near-end must still be evaluated on every
    // effect run (loadNextPage is idempotent on the pager side).
    const onNearEnd2 = vi.fn();
    const onRangeChange2 = vi.fn();
    await act(async () => {
      getRoot().render(
        <VirtualGalleryGrid
          items={items}
          thumbnails={new Map()}
          loadingThumbs={new Set()}
          onItemClick={vi.fn()}
          onRangeChange={onRangeChange2}
          onNearEnd={onNearEnd2}
        />
      );
      await flush();
    });

    expect(onNearEnd2).toHaveBeenCalledTimes(1);
    expect(onRangeChange2).not.toHaveBeenCalled(); // range itself is unchanged
  });

  it('re-reports the range when items reload (new array identity, same range)', async () => {
    // The dedupe key is (items identity + range key): a gallery refresh
    // rebuilds the items array at the same scroll position, and the grid must
    // re-report so the scheduler re-requests thumbnails for the viewport.
    const CONTAINER_HEIGHT = 360;
    const onRangeChange = vi.fn();

    await act(async () => {
      getRoot().render(
        <VirtualGalleryGrid
          items={makeItems(27)}
          thumbnails={new Map()}
          loadingThumbs={new Set()}
          onItemClick={vi.fn()}
          onRangeChange={onRangeChange}
        />
      );
      await flush();
    });

    const gridContainer = getContainer().querySelector('[data-testid="virtual-grid-container"]');
    expect(gridContainer).toBeTruthy();
    if (gridContainer) {
      act(() => {
        resizeMock.triggerResize(gridContainer, CONTAINER_HEIGHT);
      });
    }
    await flush();

    const callsAfterFirst = onRangeChange.mock.calls.length;
    expect(callsAfterFirst).toBeGreaterThanOrEqual(1);
    const firstVisible = onRangeChange.mock.calls[callsAfterFirst - 1][0];

    // Reload: same content, same range key — but a fresh items array.
    await act(async () => {
      getRoot().render(
        <VirtualGalleryGrid
          items={makeItems(27)}
          thumbnails={new Map()}
          loadingThumbs={new Set()}
          onItemClick={vi.fn()}
          onRangeChange={onRangeChange}
        />
      );
      await flush();
    });

    expect(onRangeChange.mock.calls.length).toBe(callsAfterFirst + 1);
    const reloadedVisible = onRangeChange.mock.calls[callsAfterFirst][0];
    expect(reloadedVisible).toEqual(firstVisible);
  });

  it('sizes the spacer from measured row-gap-aware metrics', async () => {
    // Wiring check: measureGridMetrics feeds padTop/pitch/padBottom into the
    // spacer height. With a resolved width of 380px the pitch is
    // (380-2-2-12)/3 + 6 = 127⅓ (NOT the hardcoded 120 fallback).
    const restore = stubInnerGridComputedStyle();
    try {
      const items = makeItems(300); // 100 rows
      await act(async () => {
        getRoot().render(
          <VirtualGalleryGrid
            items={items}
            thumbnails={new Map()}
            loadingThumbs={new Set()}
            onItemClick={vi.fn()}
          />
        );
        await flush();
      });

      const gridContainer = getContainer().querySelector('[data-testid="virtual-grid-container"]');
      expect(gridContainer).toBeTruthy();
      if (gridContainer) {
        act(() => {
          resizeMock.triggerResize(gridContainer, 360);
        });
      }
      await flush();

      const spacer = gridContainer!.querySelector(':scope > div') as HTMLElement | null;
      expect(spacer).toBeTruthy();
      const pitch = (380 - 2 - 2 - 2 * 6) / 3 + 6;
      expect(parseFloat(spacer!.style.height)).toBeCloseTo(4 + 100 * pitch + 6, 1);
    } finally {
      restore();
    }
  });

  it('scrollToIndex compensates with the measured padTop and pitch', async () => {
    const restore = stubInnerGridComputedStyle();
    try {
      const handleRef: { current: VirtualGalleryGridHandle | null } = { current: null };
      await act(async () => {
        getRoot().render(
          <VirtualGalleryGrid
            ref={handleRef}
            items={makeItems(27)}
            thumbnails={new Map()}
            loadingThumbs={new Set()}
            onItemClick={vi.fn()}
          />
        );
        await flush();
      });

      const gridContainer = getContainer().querySelector('[data-testid="virtual-grid-container"]');
      expect(gridContainer).toBeTruthy();

      // jsdom lacks Element.scrollTo; stub it to capture the target.
      let capturedTop = -1;
      gridContainer!.scrollTo = ((opts: { top?: number }) => {
        capturedTop = opts?.top ?? -1;
      }) as Element['scrollTo'];

      act(() => {
        handleRef.current?.scrollToIndex(3);
      });

      // index 3 → row 1 → top = padTop(4) + 1 × pitch(127⅓)
      const pitch = (380 - 2 - 2 - 2 * 6) / 3 + 6;
      expect(capturedTop).toBeCloseTo(4 + 1 * pitch, 1);
    } finally {
      restore();
    }
  });

  it('remeasures row metrics when the ResizeObserver fires', async () => {
    const items = makeItems(300); // 100 rows
    await act(async () => {
      getRoot().render(
        <VirtualGalleryGrid
          items={items}
          thumbnails={new Map()}
          loadingThumbs={new Set()}
          onItemClick={vi.fn()}
        />
      );
      await flush();
    });

    const gridContainer = getContainer().querySelector('[data-testid="virtual-grid-container"]');
    const innerGrid = getContainer().querySelector('[data-testid="virtual-grid-inner"]');
    expect(gridContainer).toBeTruthy();
    expect(innerGrid).toBeTruthy();
    const spacerHeight = () =>
      parseFloat((gridContainer!.querySelector(':scope > div') as HTMLElement).style.height);

    // jsdom cannot resolve the grid width at mount → fallback pitch
    // 120 (DEFAULT_GRID_METRICS).
    expect(spacerHeight()).toBeCloseTo(4 + 100 * 120 + 6, 5);

    // The real layout arrives (width becomes resolvable): the observer must
    // remeasure and re-render the spacer with the row-gap-aware pitch.
    const restore = stubInnerGridComputedStyle();
    try {
      act(() => {
        resizeMock.triggerResize(innerGrid!, 0);
      });
      await flush();

      const pitch = (380 - 2 - 2 - 2 * 6) / 3 + 6;
      expect(spacerHeight()).toBeCloseTo(4 + 100 * pitch + 6, 1);
    } finally {
      restore();
    }
  });
});
