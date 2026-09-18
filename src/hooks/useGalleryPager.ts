/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useCallback, useMemo, useRef, useState } from 'react';
import type { MediaItemDto, MediaCursor, MediaPageResponse } from '../types';
import { listMediaPage, GALLERY_PAGE_SIZE } from '../services/gallery-media-v2';

interface LoadAllOptions {
  /**
   * Epoch-ms bound (any timestamp inside the target capture day works).
   * When set, loading stops once the loaded range fully covers that capture
   * day — i.e. an item from a strictly older day has been fetched (items are
   * sorted dateDesc) — plus `marginPages` extra pages. Omit to load until
   * the cursor is exhausted (complete list, e.g. the date-jump picker's
   * full day list).
   */
  untilMs?: number;
  /** Extra pages fetched after the page that covers `untilMs` (default 2). */
  marginPages?: number;
}

interface UseGalleryPagerResult {
  items: MediaItemDto[];
  cursor: MediaCursor;
  totalCount: number;
  isLoading: boolean;
  error: string | null;
  loadNextPage: () => Promise<void>;
  reload: () => Promise<void>;
  /**
   * Load remaining pages. Without options, loads until the cursor is
   * exhausted (used to build the complete date list). With `untilMs`,
   * stops once the loaded range fully covers that capture day plus
   * `marginPages` (default 2) extra pages — enough to make the target
   * day's content visible while downward infinite scrolling keeps working
   * from the preserved cursor (everything above the target stays loaded,
   * so scrolling back up is unaffected).
   */
  loadAll: (opts?: LoadAllOptions) => Promise<void>;
  removeItems: (mediaIds: Set<string>) => void;
  addItems: (items: MediaItemDto[]) => void;
}

/** Default margin pages fetched past the page covering `untilMs`. */
const LOAD_ALL_MARGIN_PAGES = 2;

/** Local-midnight floor of an epoch-ms timestamp (capture-day granularity). */
function dayFloorMs(ms: number): number {
  const d = new Date(ms);
  return new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
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
  // 三类操作各自独立防重：翻页 / 刷新 / 全量加载互不共享 in-flight 标记 ——
  // 旧实现共享一个 inflightRef，会把"滚动分页在飞时的刷新/全量加载"静默
  // 丢弃。现在 reload/loadAll 可随时执行并取代在飞的分页。
  const pageInflightRef = useRef(false);
  const reloadInflightRef = useRef(false);
  const loadAllInflightRef = useRef(false);
  // 代际计数：每次 reload/loadAll 递增。分页请求携带发起时的代际，完成后
  // 若代际已过时（被刷新取代）则丢弃本次结果不写 state —— 与下方 stale
  // cursor 自愈同构的"过时即弃"模式，只是判据从游标错误换成代际不匹配。
  const revisionRef = useRef(0);
  const cursorRef = useRef<MediaCursor>(null);
  // `items` 状态的 ref 镜像：稳定回调（loadNextPage 守卫等）需读"当前值"，
  // 避免闭包里的陈旧快照。所有 items 变更点同步维护此 ref。
  const itemsRef = useRef<MediaItemDto[]>([]);

  const fetchPage = useCallback(async (pageCursor: MediaCursor, revision: number): Promise<MediaPageResponse | null> => {
    const response = await listMediaPage({
      cursor: pageCursor,
      pageSize: GALLERY_PAGE_SIZE,
      sort: 'dateDesc',
    });

    if (revision !== revisionRef.current) {
      // 已被更新的 reload/loadAll 取代：state 归新刷新所有，此处写入会把
      // 过时的分页数据拼回新列表（seenMediaIds 也已随之重置）。
      return null;
    }

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
    return response;
  }, []);

  const loadNextPage = useCallback(async () => {
    if (isLoading || pageInflightRef.current) {
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

    const revision = revisionRef.current;
    pageInflightRef.current = true;
    setIsLoading(true);
    setError(null);

    try {
      await fetchPage(cursor, revision);
    } catch (err) {
      if (isStaleCursorError(err)) {
        if (revision !== revisionRef.current) {
          // 已被刷新取代：刷新自身会重建首页状态，无需（也不得）再自愈。
          return;
        }
        setCursor(null);
        cursorRef.current = null;
        setItems([]);
        itemsRef.current = [];

        try {
          await fetchPage(null, revision);
        } catch (innerErr) {
          if (revision === revisionRef.current) {
            setError(innerErr instanceof Error ? innerErr.message : 'Failed to reload after stale cursor');
          }
        }
      } else if (revision === revisionRef.current) {
        setError(err instanceof Error ? err.message : 'Failed to load page');
      }
    } finally {
      pageInflightRef.current = false;
      // isLoading 由"最新代际"的操作负责复位：若已被 reload/loadAll 取代，
      // 复位会侵吞取代者的加载窗口（items 已被重置为 []，loading 掉线会
      // 让空态判断闪烁）。
      if (revision === revisionRef.current) {
        setIsLoading(false);
      }
    }
  }, [cursor, isLoading, fetchPage]);

  const reload = useCallback(async () => {
    if (reloadInflightRef.current) {
      return;
    }

    // 递增代际即取代所有在飞分页（含 loadNextPage 的自愈路径）：它们的
    // 结果会在完成时被 fetchPage 的代际检查丢弃。
    const revision = ++revisionRef.current;
    reloadInflightRef.current = true;
    setIsLoading(true);
    setError(null);
    setItems([]);
    itemsRef.current = [];
    setCursor(null);
    cursorRef.current = null;
    setTotalCount(0);
    seenMediaIdsRef.current = new Set();

    try {
      await fetchPage(null, revision);
    } catch (err) {
      if (revision === revisionRef.current) {
        setError(err instanceof Error ? err.message : 'Failed to load page');
      }
    } finally {
      reloadInflightRef.current = false;
      if (revision === revisionRef.current) {
        setIsLoading(false);
      }
    }
  }, [fetchPage]);

  // Load remaining pages until the cursor is exhausted — or, when `untilMs`
  // is given, until the loaded range covers the target capture day plus a
  // small margin. Unbounded mode feeds the date-jump picker's complete day
  // list; bounded mode is for "jump to this date": the target day only needs
  // everything from the newest item down past that day (dateDesc sort), and
  // the preserved cursor keeps downward infinite scrolling working.
  const loadAll = useCallback(async (opts?: LoadAllOptions) => {
    if (loadAllInflightRef.current) {
      return;
    }

    const untilMs = opts?.untilMs;
    const marginPages = Math.max(0, opts?.marginPages ?? LOAD_ALL_MARGIN_PAGES);

    const revision = ++revisionRef.current;
    loadAllInflightRef.current = true;
    setIsLoading(true);
    setError(null);

    try {
      // 代际检查前置到循环条件：被 reload/loadAll 取代后不再继续拉剩余页。
      // cursor=null 且从未加载过数据（典型：首载分页刚被本 loadAll 取代）时
      // 从头拉第一页；fetchedNullCursor 防止空库时对 null 游标无限重拉。
      let fetchedNullCursor = false;
      // 有界模式：目标日被"覆盖"指加载范围已越过该日（拉到严格更早日的一
      // 条 item —— dateDesc 排序下即该日全部条目已加载）。覆盖后仅再拉
      // marginPages 页余量即停；无界模式（untilMs 未提供）保持拉到底。
      let covered = false;
      let marginFetched = 0;
      if (untilMs != null) {
        const oldestLoaded = itemsRef.current[itemsRef.current.length - 1];
        covered = oldestLoaded != null && dayFloorMs(oldestLoaded.dateModifiedMs) < dayFloorMs(untilMs);
      }
      while (revisionRef.current === revision) {
        if (untilMs != null && covered && marginFetched >= marginPages) {
          break;
        }
        const pageCursor = cursorRef.current;
        if (pageCursor === null && (itemsRef.current.length > 0 || fetchedNullCursor)) {
          break;
        }
        const page = await fetchPage(pageCursor, revision);
        if (pageCursor === null) {
          fetchedNullCursor = true;
        }
        if (page === null) {
          // 已被取代：下一轮循环条件即退出。
          continue;
        }
        // 余量计数在覆盖判定之前：本页拉取时若尚未覆盖，则本页是（潜在的）
        // 覆盖页本身，不计入余量；余量只数覆盖页之后的页。
        if (untilMs != null && covered) {
          marginFetched += 1;
        }
        if (untilMs != null && !covered) {
          const oldest = page.items.length > 0 ? page.items[page.items.length - 1] : undefined;
          if (oldest != null && dayFloorMs(oldest.dateModifiedMs) < dayFloorMs(untilMs)) {
            covered = true;
          }
        }
      }
    } catch (err) {
      if (revision === revisionRef.current) {
        setError(err instanceof Error ? err.message : 'Failed to load all media');
      }
    } finally {
      loadAllInflightRef.current = false;
      if (revision === revisionRef.current) {
        setIsLoading(false);
      }
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
