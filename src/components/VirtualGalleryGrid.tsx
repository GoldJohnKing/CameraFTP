/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { type TouchEvent, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { Check, Loader2 } from 'lucide-react';
import type { MediaItemDto } from '../types/gallery-v2';

const COLUMNS = 3;
const DEFAULT_ROW_HEIGHT = 120;
const DEFAULT_OVERSCAN_ROWS = 3;

export interface VirtualGalleryGridProps {
  items: MediaItemDto[];
  thumbnails: Map<string, string>;
  loadingThumbs: Set<string>;
  onItemClick: (item: MediaItemDto) => void;
  onRangeChange?: (visibleIds: string[], nearbyIds: string[]) => void;
  rowHeight?: number;
  overscanRows?: number;
  /** Selection mode overlay support */
  isSelectionMode?: boolean;
  selectedIds?: Set<string>;
  deletingIds?: Set<string>;
  onTouchStart?: (mediaId: string, event: TouchEvent) => void;
  onTouchMove?: (event: TouchEvent) => void;
  onTouchEnd?: () => void;
}

export function VirtualGalleryGrid({
  items,
  thumbnails,
  loadingThumbs,
  onItemClick,
  onRangeChange,
  rowHeight = DEFAULT_ROW_HEIGHT,
  overscanRows = DEFAULT_OVERSCAN_ROWS,
  isSelectionMode = false,
  selectedIds,
  deletingIds,
  onTouchStart,
  onTouchMove,
  onTouchEnd,
}: VirtualGalleryGridProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const [containerHeight, setContainerHeight] = useState(0);

  const totalRows = Math.ceil(items.length / COLUMNS);
  const totalHeight = totalRows * rowHeight;

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

  // Handle scroll
  const handleScroll = useCallback(() => {
    const el = containerRef.current;
    if (!el) return;
    setScrollTop(el.scrollTop);
  }, []);

  // Calculate visible range
  const { startRow, endRow, visibleStartRow, visibleEndRow } = useMemo(() => {
    const visibleStartRow = Math.max(0, Math.floor(scrollTop / rowHeight));
    const visibleEndRow = Math.min(
      totalRows - 1,
      Math.floor((scrollTop + containerHeight) / rowHeight)
    );

    const startRow = Math.max(0, visibleStartRow - overscanRows);
    const endRow = Math.min(totalRows - 1, visibleEndRow + overscanRows);

    return { startRow, endRow, visibleStartRow, visibleEndRow };
  }, [scrollTop, containerHeight, rowHeight, totalRows, overscanRows]);

  // Build visible items slice
  const visibleItems = useMemo(() => {
    const startIdx = startRow * COLUMNS;
    const endIdx = Math.min(items.length, (endRow + 1) * COLUMNS);
    return items.slice(startIdx, endIdx);
  }, [items, startRow, endRow]);

  // Report range changes
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

    console.log(`[VGrid] onRangeChange: visible=${visibleIds.length} nearby=${nearbyIds.length} containerH=${containerHeight} totalRows=${totalRows}`);
    onRangeChange(visibleIds, nearbyIds);
  }, [items, visibleStartRow, visibleEndRow, startRow, endRow, onRangeChange, containerHeight, totalRows]);

  const offsetY = startRow * rowHeight;

  return (
    <div
      ref={containerRef}
      className="w-full h-full overflow-auto"
      data-testid="virtual-grid-container"
      onScroll={handleScroll}
    >
      <div style={{ height: totalHeight, position: 'relative' }}>
        <div
          className="grid grid-cols-3 gap-1.5 px-0.5 pb-1.5"
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

            return (
              <div
                key={item.mediaId}
                data-media-id={item.mediaId}
                data-grid-index={globalIdx}
                onClick={() => onItemClick(item)}
                onTouchStart={onTouchStart ? (e) => onTouchStart(item.mediaId, e) : undefined}
                onTouchMove={onTouchMove}
                onTouchEnd={onTouchEnd}
                onTouchCancel={onTouchEnd}
                onContextMenu={(e) => e.preventDefault()}
                className={`aspect-square bg-gray-100 rounded-lg overflow-hidden cursor-pointer hover:opacity-90 transition-opacity duration-200 relative select-none ${
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
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
