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
 *
 * For RAW files (NEF, CR2, ARW, etc.), Android's ImageDecoder cannot reliably
 * read EXIF orientation from the RAW file's TIFF structure. After the Kotlin
 * thumbnail pipeline saves a JPEG thumbnail (without EXIF), this scheduler
 * calls Rust's nom_exif to read the correct orientation from the RAW file and
 * injects it into the saved JPEG. The browser's `imageOrientation: from-image`
 * CSS then applies the rotation.
 */

import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  enqueueThumbnails,
  cancelThumbnailRequests,
  registerThumbnailListener,
  unregisterThumbnailListener,
} from '../services/gallery-media-v2';
import type { ThumbRequest, ThumbResult } from '../types';
import { isRawFile } from '../utils/raw';

const DEBOUNCE_MS = 60;
const VIEW_ID = 'gallery-grid';
const LISTENER_ID = 'thumbnail-scheduler';
const SIZE_BUCKET = 's';

/**
 * LRU cap for the in-memory thumbnail URL cache (mediaId → URL). The Map
 * only ever grew before, pinning every thumbnail ever shown for the
 * component's lifetime. Entries beyond this cap are evicted oldest-first
 * (Map iteration order = insertion order); consumers treat an evicted entry
 * exactly like a never-loaded one — the placeholder shows and the scheduler
 * re-requests the thumbnail when the item scrolls back into view (the
 * native layer serves it from its disk cache).
 */
const THUMBNAIL_CACHE_MAX = 300;

/**
 * Immutable upsert of a thumbnail entry with LRU eviction: an entry whose
 * URL is unchanged is returned as-is — the early return intentionally does
 * NOT refresh recency (a same-URL re-delivery must not churn the LRU order
 * or cause a needless re-render). A genuinely changed URL moves the entry
 * to the back (most-recent), and the oldest entries are evicted while the
 * map exceeds the cap.
 */
function upsertThumbnailEntry(
  prev: Map<string, string>,
  mediaId: string,
  url: string,
): Map<string, string> {
  if (prev.get(mediaId) === url) {
    return prev;
  }
  const next = new Map(prev);
  // Delete-then-set so a refreshed entry moves to the back of the
  // insertion-ordered Map (a plain re-set keeps its original position).
  next.delete(mediaId);
  next.set(mediaId, url);
  while (next.size > THUMBNAIL_CACHE_MAX) {
    const oldest = next.keys().next().value;
    if (oldest === undefined) break;
    next.delete(oldest);
  }
  return next;
}

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
  filePath: string | null;
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
  // 内容键缓存（mediaId → wantedKey，即 mediaId|dateModifiedMs|sizeBucket）：
  // 缩略图 URL 缓存只按 mediaId 命中，无法感知同路径文件被重新上传
  // （同 mediaId、新 mtime）的情况。记录每条缓存由哪个内容键生成后，
  // tryEnqueue 就能识别"缓存键已变"并重新请求，让缩略图缓存得以在
  // SWR 式 reload（不清空缓存）下保持内容正确。
  const cachedKeysRef = useRef<Map<string, string>>(new Map());
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pendingRef = useRef<{ visibleIds: string[]; nearbyIds: string[] } | null>(null);
  const debounceMsRef = useRef(debounceMs);
  debounceMsRef.current = debounceMs;
  const thumbnailsRef = useRef(thumbnails);
  thumbnailsRef.current = thumbnails;

  /**
   * Register media metadata so the scheduler can build ThumbRequests.
   * Called by the consumer (e.g. GalleryCard) when media items are loaded.
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
        // 记录本条缓存缩略图的内容键（非 RAW 与 RAW 两条路径都会落地
        // URL，此处统一先行记录），供 tryEnqueue 做内容键比对。
        cachedKeysRef.current.set(result.mediaId, active.wantedKey);
        const isRaw = media.filePath ? isRawFile(media.filePath) : false;
        if (isRaw && media.filePath) {
          // Defer cleanup to the async orientation fix
          void fixRawOrientation(
            result.mediaId,
            result.requestId,
            media.filePath,
            result.localPath,
          );
        } else {
          const url = convertFileSrc(result.localPath);
          setThumbnails((prev) => upsertThumbnailEntry(prev, result.mediaId, url));
          cleanupRequest(result.requestId, result.mediaId);
        }
      } else {
        cleanupRequest(result.requestId, result.mediaId);
      }

      if (result.status === 'failed' && !isRetryable(result.errorCode)) {
        failedMediaRef.current.add(result.mediaId);
      }
    };

    const cleanupRequest = (requestId: string, mediaId: string) => {
      activeRequestsRef.current.delete(requestId);
      setLoadingThumbs((prev) => {
        const next = new Set(prev);
        next.delete(mediaId);
        return next;
      });
    };

    /** Read orientation from RAW file via Rust nom_exif and inject into thumbnail JPEG. */
    const fixRawOrientation = async (
      mediaId: string,
      requestId: string,
      rawFilePath: string,
      thumbnailPath: string,
    ) => {
      try {
        const orientation = await invoke<number>('get_raw_orientation', { filePath: rawFilePath });
        if (orientation > 1) {
          await invoke<boolean>('inject_exif_orientation', { thumbnailPath, orientation });
        }
        const url = convertFileSrc(thumbnailPath);
        setThumbnails((prev) => upsertThumbnailEntry(prev, mediaId, url));
      } catch (e) {
        // Orientation fix failed — display thumbnail as-is (better than nothing)
        console.warn(`Failed to fix RAW orientation for ${rawFilePath}:`, e);
        const url = convertFileSrc(thumbnailPath);
        setThumbnails((prev) => upsertThumbnailEntry(prev, mediaId, url));
      } finally {
        cleanupRequest(requestId, mediaId);
      }
    };

    void registerThumbnailListener(VIEW_ID, LISTENER_ID, handleResult).catch(() => {});

    return () => {
      void unregisterThumbnailListener(LISTENER_ID).catch(() => {});
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
        void cancelThumbnailRequests(toCancel).catch(() => {});
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

        // Already have a thumbnail loaded — and it was built from the SAME
        // content key. A cached thumb whose mediaId|dateModifiedMs|sizeBucket
        // changed (e.g. FTP re-upload of the same path → same mediaId, new
        // mtime) is re-requested; upsertThumbnailEntry replaces the URL.
        if (thumbnailsRef.current.has(mediaId) && cachedKeysRef.current.get(mediaId) === wantedKey) return;

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
    [],
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
      void cancelThumbnailRequests(toCancel).catch(() => {});
      for (const id of toCancel) {
        activeRequestsRef.current.delete(id);
      }
    }

    for (const id of mediaIds) {
      mediaMapRef.current.delete(id);
      failedMediaRef.current.delete(id);
      cachedKeysRef.current.delete(id);
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
      void cancelThumbnailRequests(allRequestIds).catch(() => {});
    }
    activeRequestsRef.current.clear();
    failedMediaRef.current.clear();
    // Also drop the accumulated thumbnail cache and registered media
    // metadata — previously these leaked across cleanup (e.g. gallery
    // refresh), pinning evicted entries and stale mediaMap lookups.
    setThumbnails(new Map());
    mediaMapRef.current.clear();
    setLoadingThumbs(new Set());
    cachedKeysRef.current.clear();
  }, []);

  /**
   * 仅清除"永久失败"标记，不清空缩略图缓存、loading 集合与媒体表：
   * 手动刷新时借此重试此前解码/权限永久失败的缩略图；已缓存的缩略图
   * 按内容键存续（内容未变即无需重取），可安全跨 reload 保留。
   */
  const resetFailures = useCallback(() => {
    failedMediaRef.current.clear();
  }, []);

  useEffect(() => {
    return cleanup;
  }, [cleanup]);

  return useMemo(
    () => ({
      thumbnails,
      loadingThumbs,
      updateViewport,
      removeThumbs,
      cleanup,
      resetFailures,
      registerMedia,
    }),
    [thumbnails, loadingThumbs, updateViewport, removeThumbs, cleanup, resetFailures, registerMedia],
  );
}
