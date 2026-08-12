/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { type TouchEvent, forwardRef, useCallback, useEffect, useImperativeHandle, useMemo, useRef, useState } from 'react';
import { Check, Loader2 } from 'lucide-react';
import type { MediaItemDto } from '../types';
import { classifyFile } from '../utils/gallery-filter';

const COLUMNS = 3;
const ROW_HEIGHT = 120;
const OVERSCAN_ROWS = 3;
const NEAR_END_THRESHOLD = 5;
const SCROLL_END_DELAY = 150;

export interface VirtualGalleryGridProps {
  items: MediaItemDto[];
  thumbnails: Map<string, string>;
  loadingThumbs: Set<string>;
  onItemClick: (item: MediaItemDto) => void;
  onRangeChange?: (visibleIds: string[], nearbyIds: string[]) => void;
  /** Selection mode overlay support */
  isSelectionMode?: boolean;
  selectedIds?: Set<string>;
  deletingIds?: Set<string>;
  onTouchStart?: (mediaId: string, event: TouchEvent, isScrolling: boolean, gridIndex: number) => void;
  onTouchMove?: (event: TouchEvent) => void;
  onTouchEnd?: () => void;
  /** Drag-select: called with the set of mediaIds in the range from anchor to finger position */
  onDragSelect?: (mediaIds: Set<string>) => void;
  /** Ref to check if drag-select is active (from useGallerySelection) */
  isDragSelectingRef?: React.RefObject<boolean>;
  /** Ref for the anchor index where drag-select started */
  dragAnchorIndexRef?: React.RefObject<number>;
  /** Called when scrolling near the end to trigger infinite scroll */
  onNearEnd?: () => void;
  /** mediaId of the cell to briefly highlight (e.g. after a date-jump). */
  highlightMediaId?: string | null;
}

/** Imperative handle exposed by the grid via ref. */
export interface VirtualGalleryGridHandle {
  /** Scroll so the item at `index` sits at the top of the viewport. */
  scrollToIndex: (index: number) => void;
}

export const VirtualGalleryGrid = forwardRef<VirtualGalleryGridHandle, VirtualGalleryGridProps>(function VirtualGalleryGrid({
  items,
  thumbnails,
  loadingThumbs,
  onItemClick,
  onRangeChange,
  isSelectionMode = false,
  selectedIds,
  deletingIds,
  onTouchStart,
  onTouchMove,
  onTouchEnd,
  onDragSelect,
  isDragSelectingRef,
  dragAnchorIndexRef,
  onNearEnd,
  highlightMediaId,
}, ref) {
  const containerRef = useRef<HTMLDivElement>(null);
  const itemsRef = useRef(items);
  itemsRef.current = items;
  useImperativeHandle(ref, () => ({
    scrollToIndex(index: number) {
      const row = Math.floor(index / COLUMNS);
      containerRef.current?.scrollTo({ top: row * ROW_HEIGHT });
    },
  }), [containerRef]);
  const [scrollTop, setScrollTop] = useState(0);
  const [containerHeight, setContainerHeight] = useState(0);
  const [isScrolling, setIsScrolling] = useState(false);
  const scrollEndTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  // The parent requests a highlight via `highlightMediaId`, but the target cell
  // may not be in the render window yet (e.g. after a multi-page jump, the scroll
  // hasn't settled so `visibleItems` still reflects the old position). We latch
  // the request and only arm the pulse once the target actually mounts; the clear
  // timer likewise runs from that moment, not from when the jump was requested.
  const [activeHighlight, setActiveHighlight] = useState<string | null>(null);
  const highlightClearRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  // Last highlight request we actually armed. The latch effect re-runs whenever
  // visibleItems changes (e.g. a gallery refresh rebuilding the items array, or
  // scrolling the target back into view), and highlightMediaId is not cleared
  // after a pulse — without this guard that re-run would replay the animation.
  // We arm at most once per request; resetting the request to null (as
  // GalleryCard does between repeat jumps to the same day) clears the guard so
  // the next pulse still fires.
  const lastArmedHighlightRef = useRef<string | null>(null);

  const totalRows = Math.ceil(items.length / COLUMNS);
  const totalHeight = totalRows * ROW_HEIGHT;

  // Observe container height
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;

    const observer = new ResizeObserver((entries) => {
      for (const entry of entries) {
        setContainerHeight(entry.contentRect.height);
      }
    });
    observer.observe(el);
    setContainerHeight(el.clientHeight);

    return () => observer.disconnect();
  }, []);

  // Native non-passive touchmove listener for drag-select scroll prevention
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;

    const handleNativeTouchMove = (event: globalThis.TouchEvent) => {
      if (!isDragSelectingRef?.current || !onDragSelect) return;

      event.preventDefault();

      const touch = event.touches[0];
      if (!touch) return;

      const element = document.elementFromPoint(touch.clientX, touch.clientY);
      if (!element) return;

      const cell = (element as HTMLElement).closest<HTMLElement>('[data-media-id]');
      if (!cell) return;

      const mediaId = cell.dataset.mediaId;
      if (!mediaId) return;

      const gridIndex = Number(cell.dataset.gridIndex);
      if (isNaN(gridIndex)) return;

      const anchorIndex = dragAnchorIndexRef?.current ?? -1;
      if (anchorIndex < 0) return;

      const startIdx = Math.min(anchorIndex, gridIndex);
      const endIdx = Math.max(anchorIndex, gridIndex);
      const rangeIds = new Set<string>();
      for (let i = startIdx; i <= endIdx; i++) {
        const item = itemsRef.current[i];
        if (item) rangeIds.add(item.mediaId);
      }
      onDragSelect(rangeIds);
    };

    el.addEventListener('touchmove', handleNativeTouchMove, { passive: false });

    return () => {
      el.removeEventListener('touchmove', handleNativeTouchMove);
    };
  }, [onDragSelect, isDragSelectingRef, dragAnchorIndexRef]);

  // Cleanup scroll timer on unmount
  useEffect(() => {
    return () => {
      if (scrollEndTimerRef.current) {
        clearTimeout(scrollEndTimerRef.current);
      }
    };
  }, []);

  // Handle scroll with state tracking
  const handleScroll = useCallback(() => {
    const el = containerRef.current;
    if (!el) return;
    setScrollTop(el.scrollTop);

    // Mark as scrolling
    setIsScrolling(true);

    // Clear previous timer
    if (scrollEndTimerRef.current) {
      clearTimeout(scrollEndTimerRef.current);
    }

    // Set timer to detect scroll end
    scrollEndTimerRef.current = setTimeout(() => {
      setIsScrolling(false);
    }, SCROLL_END_DELAY);
  }, []);

  // Calculate visible range
  const { startRow, endRow, visibleStartRow, visibleEndRow } = useMemo(() => {
    const visibleStartRow = Math.max(0, Math.floor(scrollTop / ROW_HEIGHT));
    const visibleEndRow = Math.min(
      totalRows - 1,
      Math.floor((scrollTop + containerHeight) / ROW_HEIGHT)
    );

    const startRow = Math.max(0, visibleStartRow - OVERSCAN_ROWS);
    const endRow = Math.min(totalRows - 1, visibleEndRow + OVERSCAN_ROWS);

    return { startRow, endRow, visibleStartRow, visibleEndRow };
  }, [scrollTop, containerHeight, totalRows]);

  // Build visible items slice
  const visibleItems = useMemo(() => {
    const startIdx = startRow * COLUMNS;
    const endIdx = Math.min(items.length, (endRow + 1) * COLUMNS);
    return items.slice(startIdx, endIdx);
  }, [items, startRow, endRow]);

  // Report range changes and trigger infinite scroll
  useEffect(() => {
    if (!onRangeChange) return;
    if (items.length === 0) return;
    // Skip if container height is not yet measured - prevents incorrect range calculation
    if (containerHeight === 0) return;

    const visibleStartIdx = visibleStartRow * COLUMNS;
    const visibleEndIdx = Math.min(items.length, (visibleEndRow + 1) * COLUMNS);
    const visibleIds = items.slice(visibleStartIdx, visibleEndIdx).map((item) => item.mediaId);

    const nearbyStartIdx = startRow * COLUMNS;
    const nearbyEndIdx = Math.min(items.length, (endRow + 1) * COLUMNS);
    const nearbyIds = items
      .slice(nearbyStartIdx, nearbyEndIdx)
      .map((item) => item.mediaId)
      .filter((id) => !visibleIds.includes(id));

    onRangeChange(visibleIds, nearbyIds);

    // Trigger infinite scroll when near the end
    if (onNearEnd && totalRows > 0) {
      const rowsRemaining = totalRows - visibleEndRow - 1;
      if (rowsRemaining <= NEAR_END_THRESHOLD) {
        onNearEnd();
      }
    }
  }, [items, visibleStartRow, visibleEndRow, startRow, endRow, onRangeChange, onNearEnd, containerHeight, totalRows]);

  // When the parent requests a highlight, arm it only once the target cell is in
  // the render window. This decouples the pulse from scroll timing: on a big
  // jump the target won't be visible until the scroll settles, so arming it
  // eagerly (and clearing on a wall-clock timer) could expire before the cell
  // ever mounts. Here we wait for visibility, then play the pulse to completion.
  //
  // The pulse fires at most ONCE per request: highlightMediaId is not cleared
  // after the pulse, so any later visibleItems change (a refresh's reload, or
  // scrolling the target back into view) would otherwise re-arm the latch and
  // replay the animation. We skip re-arming a request we've already armed, and
  // clear the guard when the request becomes null so a repeat jump to the same
  // day still pulses.
  useEffect(() => {
    const requested = highlightMediaId;
    if (!requested) {
      lastArmedHighlightRef.current = null;
      return;
    }
    if (lastArmedHighlightRef.current === requested) return;
    const inWindow = visibleItems.some((item) => item.mediaId === requested);
    if (!inWindow) return;
    // Target is mounted — arm the pulse and mark this request as handled.
    lastArmedHighlightRef.current = requested;
    setActiveHighlight(requested);
    clearTimeout(highlightClearRef.current ?? undefined);
    highlightClearRef.current = setTimeout(() => setActiveHighlight(null), 850);
  }, [highlightMediaId, visibleItems]);

  // Cancel an in-flight highlight when the dataset is emptied — most importantly
  // a gallery refresh, whose pager.reload() sets items to []. Without this, the
  // armed activeHighlight survives the empty→refill and re-renders the overlay
  // on the remounted cell, so a refresh during the pulse looks like it
  // re-triggered the animation. lastArmedHighlightRef is left set so the refill
  // doesn't re-arm the pulse either.
  useEffect(() => {
    if (activeHighlight == null) return;
    if (items.length > 0) return;
    setActiveHighlight(null);
    clearTimeout(highlightClearRef.current ?? undefined);
  }, [items.length, activeHighlight]);

  // Clear the highlight pulse timer on unmount.
  useEffect(() => () => clearTimeout(highlightClearRef.current ?? undefined), []);

  const offsetY = startRow * ROW_HEIGHT;

  return (
    <div
      ref={containerRef}
      className="w-full h-full overflow-auto"
      data-testid="virtual-grid-container"
      onScroll={handleScroll}
    >
      <div className="relative" style={{ height: totalHeight }}>
        <div
          className="grid grid-cols-3 gap-1.5 px-0.5 pt-1 pb-1.5"
          style={{
            position: 'absolute',
            top: offsetY,
            left: 0,
            right: 0,
          }}
          data-testid="virtual-grid-inner"
        >
          {visibleItems.map((item, idx) => {
            const globalIdx = startRow * COLUMNS + idx;
            const thumbnail = thumbnails.get(item.mediaId);
            const isLoadingThumb = loadingThumbs.has(item.mediaId);
            const isSelected = selectedIds?.has(item.mediaId) ?? false;
            const isDeleting = deletingIds?.has(item.mediaId) ?? false;
            const typeCategory = classifyFile(item);

            return (
              <div
                key={item.mediaId}
                data-media-id={item.mediaId}
                data-grid-index={globalIdx}
                onClick={() => onItemClick(item)}
                onTouchStart={onTouchStart ? (e) => onTouchStart(item.mediaId, e, isScrolling, globalIdx) : undefined}
                onTouchMove={onTouchMove}
                onTouchEnd={onTouchEnd}
                onTouchCancel={onTouchEnd}
                onContextMenu={(e) => e.preventDefault()}
                className={`aspect-square bg-gray-100 rounded-lg overflow-hidden cursor-pointer [@media(hover:hover)]:hover:opacity-90 active:opacity-90 transition-opacity duration-200 relative select-none ${
                  isSelectionMode && isSelected ? 'ring-2 ring-blue-500' : ''
                } ${isDeleting ? 'scale-[0.88] opacity-0' : 'scale-100 opacity-100'}`}
                style={{
                  transitionDuration: isDeleting ? '180ms' : undefined,
                }}
              >
                {thumbnail ? (
                  <img
                    src={thumbnail}
                    alt={item.mediaId}
                    className="w-full h-full object-cover pointer-events-none"
                    style={{ imageOrientation: 'from-image' }}
                    loading="lazy"
                    draggable={false}
                  />
                ) : isLoadingThumb ? (
                  <div className="w-full h-full flex items-center justify-center bg-gray-200">
                    <Loader2 className="w-6 h-6 text-gray-400 animate-spin" />
                  </div>
                ) : (
                  <div className="w-full h-full flex items-center justify-center">
                    <div className="w-8 h-8 bg-gray-200 rounded animate-pulse" />
                  </div>
                )}

                {isSelectionMode && (
                  <div className={`absolute top-2 left-2 w-6 h-6 rounded-full flex items-center justify-center ${
                    isSelected
                      ? 'bg-blue-500'
                      : 'bg-black/30 border-2 border-white/70'
                  }`}>
                    {isSelected && (
                      <Check className="w-4 h-4 text-white" />
                    )}
                  </div>
                )}

                {(typeCategory === 'raw' || typeCategory === 'heif') && (
                  <div
                    data-testid="type-label"
                    data-type-category={typeCategory}
                    className="absolute top-1 right-1 pointer-events-none rounded bg-gray-500/50 px-1 py-0.5 text-[10px] font-semibold leading-none text-white"
                  >
                    {typeCategory === 'raw' ? 'RAW' : 'HEIF'}
                  </div>
                )}

                {activeHighlight === item.mediaId && (
                  <>
                    {/* Whole-cell white flash — briefly brightens the photo.
                        Opacity-only; synced with the ring (same 0.8s/easing). */}
                    <div className="absolute inset-0 rounded-lg pointer-events-none bg-white animate-highlight-jump-flash" />
                    {/* Dual-tone ring ping — inset box-shadow survives the cell's
                        overflow-hidden and reads on any photo; carries the test
                        id used by the highlight regression tests. */}
                    <div
                      data-testid="highlight-overlay"
                      className="absolute inset-0 rounded-lg pointer-events-none animate-highlight-jump-ring"
                    />
                  </>
                )}
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
});
