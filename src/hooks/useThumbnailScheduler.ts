/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

/**
 * useThumbnailScheduler — Viewport-priority thumbnail request scheduler.
 *
 * Manages a batched, debounced thumbnail pipeline for the V2 gallery.
 * Enqueues visible items as high priority, nearby items as medium priority,
 * and cancels requests that scroll out of range.
 */

import { convertFileSrc } from '@tauri-apps/api/core';
import { useCallback, useEffect, useRef, useState } from 'react';
import {
  enqueueThumbnails,
  cancelThumbnailRequests,
  registerThumbnailListener,
  unregisterThumbnailListener,
} from '../services/gallery-media-v2';
import type { ThumbRequest, ThumbResult } from '../types';

const DEBOUNCE_MS = 60;
const VIEW_ID = 'gallery-grid';
const LISTENER_ID = 'thumbnail-scheduler';
const SIZE_BUCKET = 's';

type WantedKey = string;

interface ActiveRequest {
  requestId: string;
  mediaId: string;
  wantedKey: WantedKey;
}

function makeWantedKey(mediaId: string, dateModifiedMs: number, sizeBucket: string): string {
  return `${mediaId}|${dateModifiedMs}|${sizeBucket}`;
}

/**
 * Determine if a failed thumbnail request should be retried based on error code.
 * Transient errors (io_transient, oom_guard) are retryable.
 * Permanent errors (decode_corrupt, permission_denied, cancelled) are not.
 */
function isRetryable(errorCode: string | undefined): boolean {
  if (!errorCode) return true;
  return errorCode === 'io_transient' || errorCode === 'oom_guard';
}

type ThumbnailSchedulerMedia = {
  mediaId: string;
  uri: string;
  dateModifiedMs: number;
};

type UseThumbnailSchedulerOptions = {
  /** Override debounce interval in ms (default: 60). Useful for testing. */
  debounceMs?: number;
};

export function useThumbnailScheduler(opts?: UseThumbnailSchedulerOptions) {
  const debounceMs = opts?.debounceMs ?? DEBOUNCE_MS;

  const [thumbnails, setThumbnails] = useState<Map<string, string>>(new Map());
  const [loadingThumbs, setLoadingThumbs] = useState<Set<string>>(new Set());

  const activeRequestsRef = useRef<Map<string, ActiveRequest>>(new Map());
  const mediaMapRef = useRef<Map<string, ThumbnailSchedulerMedia>>(new Map());
  const failedMediaRef = useRef<Set<string>>(new Set());
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pendingRef = useRef<{ visibleIds: string[]; nearbyIds: string[] } | null>(null);
  const debounceMsRef = useRef(debounceMs);
  debounceMsRef.current = debounceMs;

  /**
   * Register media metadata so the scheduler can build ThumbRequests.
   * Called internally when media items are loaded.
   */
  const registerMedia = useCallback((items: ThumbnailSchedulerMedia[]) => {
    for (const item of items) {
      mediaMapRef.current.set(item.mediaId, item);
    }
  }, []);

  // ---- dispatch handler ----
  useEffect(() => {
    const handleResult = (result: ThumbResult) => {
      const active = activeRequestsRef.current.get(result.requestId);
      if (!active) {
        return;
      }

      const media = mediaMapRef.current.get(result.mediaId);
      if (!media) return;

      const currentWantedKey = makeWantedKey(media.mediaId, media.dateModifiedMs, SIZE_BUCKET);
      if (active.wantedKey !== currentWantedKey) return;

      if (result.status === 'ready' && result.localPath) {
        const url = result.localPath.startsWith('data:image/')
          ? result.localPath
          : convertFileSrc(result.localPath);
        setThumbnails((prev) => new Map(prev).set(result.mediaId, url));
      }

      activeRequestsRef.current.delete(result.requestId);
      setLoadingThumbs((prev) => {
        const next = new Set(prev);
        next.delete(result.mediaId);
        return next;
      });

      if (result.status === 'failed') {
        if (isRetryable(result.errorCode)) {
          // Retryable: will be re-enqueued on next viewport tick
        } else {
          // Permanent failure: track so we don't re-enqueue
          failedMediaRef.current.add(result.mediaId);
        }
      }
    };

    void registerThumbnailListener(VIEW_ID, LISTENER_ID, handleResult);

    return () => {
      void unregisterThumbnailListener(LISTENER_ID);
    };
  }, []);

  // ---- debounced viewport processing ----
  const processViewport = useCallback(
    (visibleIds: string[], nearbyIds: string[]) => {
      const visibleSet = new Set(visibleIds);
      const nearbySet = new Set(nearbyIds);
      const inRange = new Set<string>([...visibleSet, ...nearbySet]);

      // Cancel requests that left both visible and nearby
      const toCancel: string[] = [];
      for (const [requestId, req] of activeRequestsRef.current) {
        if (!inRange.has(req.mediaId)) {
          toCancel.push(requestId);
        }
      }
      if (toCancel.length > 0) {
        void cancelThumbnailRequests(toCancel);
        for (const id of toCancel) {
          const req = activeRequestsRef.current.get(id);
          if (req) {
            activeRequestsRef.current.delete(id);
            setLoadingThumbs((prev) => {
              const next = new Set(prev);
              next.delete(req.mediaId);
              return next;
            });
          }
        }
      }

      // Build new requests for items that need thumbnails
      const newReqs: ThumbRequest[] = [];

      const tryEnqueue = (mediaId: string, priority: 'visible' | 'nearby') => {
        const media = mediaMapRef.current.get(mediaId);
        if (!media) return;

        // Skip permanently failed items
        if (failedMediaRef.current.has(mediaId)) return;

        const wantedKey = makeWantedKey(media.mediaId, media.dateModifiedMs, SIZE_BUCKET);

        // Already have a matching active request
        for (const req of activeRequestsRef.current.values()) {
          if (req.mediaId === mediaId && req.wantedKey === wantedKey) return;
        }

        // Already have a thumbnail loaded
        if (thumbnails.has(mediaId)) return;

        const requestId = `${mediaId}-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
        activeRequestsRef.current.set(requestId, { requestId, mediaId, wantedKey });

        newReqs.push({
          requestId,
          mediaId: media.mediaId,
          uri: media.uri,
          dateModifiedMs: media.dateModifiedMs,
          sizeBucket: SIZE_BUCKET,
          priority,
          viewId: VIEW_ID,
        });

        setLoadingThumbs((prev) => new Set(prev).add(mediaId));
      };

      // Visible items first (high priority)
      for (const id of visibleIds) {
        tryEnqueue(id, 'visible');
      }

      // Nearby items (medium priority)
      for (const id of nearbyIds) {
        if (!visibleSet.has(id)) {
          tryEnqueue(id, 'nearby');
        }
      }

      if (newReqs.length > 0) {
        void enqueueThumbnails(newReqs).catch(() => {});
      }
    },
    [thumbnails],
  );

  // ---- public actions ----

  const updateViewport = useCallback(
    (visibleIds: string[], nearbyIds: string[]) => {
      pendingRef.current = { visibleIds, nearbyIds };

      if (debounceRef.current !== null) {
        clearTimeout(debounceRef.current);
      }

      debounceRef.current = setTimeout(() => {
        debounceRef.current = null;
        const pending = pendingRef.current;
        if (pending) {
          pendingRef.current = null;
          processViewport(pending.visibleIds, pending.nearbyIds);
        }
      }, debounceMsRef.current);
    },
    [processViewport],
  );

  const removeThumbs = useCallback((mediaIds: Set<string>) => {
    setThumbnails((prev) => {
      const next = new Map(prev);
      for (const id of mediaIds) {
        next.delete(id);
      }
      return next;
    });

    setLoadingThumbs((prev) => {
      const next = new Set(prev);
      for (const id of mediaIds) {
        next.delete(id);
      }
      return next;
    });

    const toCancel: string[] = [];
    for (const [requestId, req] of activeRequestsRef.current) {
      if (mediaIds.has(req.mediaId)) {
        toCancel.push(requestId);
      }
    }
    if (toCancel.length > 0) {
      void cancelThumbnailRequests(toCancel);
      for (const id of toCancel) {
        activeRequestsRef.current.delete(id);
      }
    }

    for (const id of mediaIds) {
      mediaMapRef.current.delete(id);
      failedMediaRef.current.delete(id);
    }
  }, []);

  const cleanup = useCallback(() => {
    if (debounceRef.current !== null) {
      clearTimeout(debounceRef.current);
      debounceRef.current = null;
    }
    pendingRef.current = null;

    const allRequestIds = [...activeRequestsRef.current.keys()];
    if (allRequestIds.length > 0) {
      void cancelThumbnailRequests(allRequestIds);
    }
    activeRequestsRef.current.clear();
    failedMediaRef.current.clear();
    setLoadingThumbs(new Set());
  }, []);

  useEffect(() => {
    return cleanup;
  }, [cleanup]);

  return {
    thumbnails,
    loadingThumbs,
    updateViewport,
    removeThumbs,
    cleanup,
    registerMedia,
  };
}
