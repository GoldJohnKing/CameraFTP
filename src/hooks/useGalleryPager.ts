/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useCallback, useRef, useState } from 'react';
import type { MediaItemDto, MediaCursor } from '../types';
import { listMediaPage } from '../services/gallery-media-v2';

const PAGE_SIZE = 120;

interface UseGalleryPagerResult {
  items: MediaItemDto[];
  cursor: MediaCursor;
  revisionToken: string;
  totalCount: number;
  isLoading: boolean;
  error: string | null;
  loadNextPage: () => Promise<void>;
  reload: () => Promise<void>;
  removeItems: (mediaIds: Set<string>) => void;
  addItems: (items: MediaItemDto[]) => void;
}

function isStaleCursorError(err: unknown): boolean {
  return err instanceof Error && err.message.includes('stale_cursor');
}

export function useGalleryPager(): UseGalleryPagerResult {
  const [items, setItems] = useState<MediaItemDto[]>([]);
  const [cursor, setCursor] = useState<MediaCursor>(null);
  const [revisionToken, setRevisionToken] = useState<string>('');
  const [totalCount, setTotalCount] = useState<number>(0);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const seenMediaIdsRef = useRef<Set<string>>(new Set());
  const inflightRef = useRef(false);

  const fetchPage = useCallback(async (pageCursor: MediaCursor): Promise<void> => {
    const response = await listMediaPage({
      cursor: pageCursor,
      pageSize: PAGE_SIZE,
      sort: 'dateDesc',
    });

    setCursor(response.nextCursor);
    setRevisionToken(response.revisionToken);
    setTotalCount(response.totalCount);

    const seen = seenMediaIdsRef.current;
    const newItems = response.items.filter((item) => {
      if (seen.has(item.mediaId)) {
        return false;
      }
      seen.add(item.mediaId);
      return true;
    });

    setItems((prev) => [...prev, ...newItems]);
  }, []);

  const loadNextPage = useCallback(async () => {
    if (isLoading || inflightRef.current) {
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
        setItems([]);

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
    setCursor(null);
    setRevisionToken('');
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

  const removeItems = useCallback((mediaIds: Set<string>) => {
    if (mediaIds.size === 0) {
      return;
    }

    setItems((prev) => {
      const next = prev.filter((item) => !mediaIds.has(item.mediaId));
      const removedCount = prev.length - next.length;
      if (removedCount > 0) {
        setTotalCount((total) => Math.max(0, total - removedCount));
      }
      return next;
    });

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
      setItems((prev) => [...itemsToAdd, ...prev]);
      setTotalCount((prev) => prev + itemsToAdd.length);
    }
  }, []);

  return {
    items,
    cursor,
    revisionToken,
    totalCount,
    isLoading,
    error,
    loadNextPage,
    reload,
    removeItems,
    addItems,
  };
}
