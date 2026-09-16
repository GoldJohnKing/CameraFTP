/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useCallback, useMemo, useRef, useState } from 'react';
import type { MediaItemDto, MediaCursor } from '../types';
import { listMediaPage, GALLERY_PAGE_SIZE } from '../services/gallery-media-v2';

interface UseGalleryPagerResult {
  items: MediaItemDto[];
  cursor: MediaCursor;
  totalCount: number;
  isLoading: boolean;
  error: string | null;
  loadNextPage: () => Promise<void>;
  reload: () => Promise<void>;
  /** Load all remaining pages (used to build the complete date list). */
  loadAll: () => Promise<void>;
  removeItems: (mediaIds: Set<string>) => void;
  addItems: (items: MediaItemDto[]) => void;
}

function isStaleCursorError(err: unknown): boolean {
  return err instanceof Error && err.message.includes('stale_cursor');
}

export function useGalleryPager(): UseGalleryPagerResult {
  const [items, setItems] = useState<MediaItemDto[]>([]);
  const [cursor, setCursor] = useState<MediaCursor>(null);
  const [totalCount, setTotalCount] = useState<number>(0);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const seenMediaIdsRef = useRef<Set<string>>(new Set());
  const inflightRef = useRef(false);
  const cursorRef = useRef<MediaCursor>(null);
  // `items` 状态的 ref 镜像：稳定回调（loadNextPage 守卫等）需读"当前值"，
  // 避免闭包里的陈旧快照。所有 items 变更点同步维护此 ref。
  const itemsRef = useRef<MediaItemDto[]>([]);

  const fetchPage = useCallback(async (pageCursor: MediaCursor): Promise<void> => {
    const response = await listMediaPage({
      cursor: pageCursor,
      pageSize: GALLERY_PAGE_SIZE,
      sort: 'dateDesc',
    });

    setCursor(response.nextCursor);
    cursorRef.current = response.nextCursor;
    setTotalCount(response.totalCount);

    const seen = seenMediaIdsRef.current;
    const newItems = response.items.filter((item) => {
      if (seen.has(item.mediaId)) {
        return false;
      }
      seen.add(item.mediaId);
      return true;
    });

    const nextItems = [...itemsRef.current, ...newItems];
    itemsRef.current = nextItems;
    setItems(nextItems);
  }, []);

  const loadNextPage = useCallback(async () => {
    if (isLoading || inflightRef.current) {
      return;
    }

    // 游标早退：数据已到底（cursor 耗尽）且已加载过数据时不再发起分页请求
    // （cursor=null 且 items 为空才是首载）。经 ref 读当前值避免闭包陈旧。
    // 防止未来消费者绕过 UI 直连 loadNextPage 时，把已耗尽的 cursor 当作
    // "从第一页开始"，被响应里的第一页 nextCursor 重新续流成自续循环。
    // reload / loadAll 各自显式管理游标，不受此守卫影响。
    if (cursorRef.current === null && itemsRef.current.length > 0) {
      return;
    }

    inflightRef.current = true;
    setIsLoading(true);
    setError(null);

    try {
      await fetchPage(cursor);
    } catch (err) {
      if (isStaleCursorError(err)) {
        setCursor(null);
        cursorRef.current = null;
        setItems([]);
        itemsRef.current = [];

        try {
          await fetchPage(null);
        } catch (innerErr) {
          setError(innerErr instanceof Error ? innerErr.message : 'Failed to reload after stale cursor');
        }
      } else {
        setError(err instanceof Error ? err.message : 'Failed to load page');
      }
    } finally {
      inflightRef.current = false;
      setIsLoading(false);
    }
  }, [cursor, isLoading, fetchPage]);

  const reload = useCallback(async () => {
    if (inflightRef.current) {
      return;
    }

    inflightRef.current = true;
    setIsLoading(true);
    setError(null);
    setItems([]);
    itemsRef.current = [];
    setCursor(null);
    cursorRef.current = null;
    setTotalCount(0);
    seenMediaIdsRef.current = new Set();

    try {
      await fetchPage(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load page');
    } finally {
      inflightRef.current = false;
      setIsLoading(false);
    }
  }, [fetchPage]);

  // Load every remaining page until the cursor is exhausted. Used to build a
  // complete list of capture dates for the date-jump picker; the virtualized
  // grid only renders the visible slice, so holding all items in memory is cheap.
  const loadAll = useCallback(async () => {
    if (inflightRef.current) {
      return;
    }

    inflightRef.current = true;
    setIsLoading(true);
    setError(null);

    try {
      while (cursorRef.current !== null) {
        await fetchPage(cursorRef.current);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load all media');
    } finally {
      inflightRef.current = false;
      setIsLoading(false);
    }
  }, [fetchPage]);

  const removeItems = useCallback((mediaIds: Set<string>) => {
    if (mediaIds.size === 0) {
      return;
    }

    const previous = itemsRef.current;
    const next = previous.filter((item) => !mediaIds.has(item.mediaId));
    const removedCount = previous.length - next.length;
    itemsRef.current = next;
    setItems(next);
    if (removedCount > 0) {
      setTotalCount((total) => Math.max(0, total - removedCount));
    }

    const seen = seenMediaIdsRef.current;
    mediaIds.forEach((id) => seen.delete(id));
  }, []);

  const addItems = useCallback((newItems: MediaItemDto[]) => {
    if (newItems.length === 0) {
      return;
    }

    const seen = seenMediaIdsRef.current;
    const itemsToAdd = newItems.filter((item) => {
      if (seen.has(item.mediaId)) {
        return false;
      }
      seen.add(item.mediaId);
      return true;
    });

    if (itemsToAdd.length > 0) {
      const next = [...itemsToAdd, ...itemsRef.current];
      itemsRef.current = next;
      setItems(next);
      setTotalCount((prev) => prev + itemsToAdd.length);
    }
  }, []);

  return useMemo(
    () => ({
      items,
      cursor,
      totalCount,
      isLoading,
      error,
      loadNextPage,
      reload,
      loadAll,
      removeItems,
      addItems,
    }),
    [items, cursor, totalCount, isLoading, error, loadNextPage, reload, loadAll, removeItems, addItems],
  );
}
