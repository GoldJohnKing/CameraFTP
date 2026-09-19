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
  /**
   * SWR（stale-while-revalidate）式刷新：刷新期间旧 items 保持可见，
   * 新首页返回后一次性原子替换（不再"先清空再回填"，消除刷新白屏闪烁）。
   * 在飞窗口内并发的增量更新（addItems/removeItems）会被合并进替换结果，
   * 而不是被整表重建清掉。
   */
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
  // reload 在飞窗口内的增量更新跟踪：addItems/removeItems 发生在首页查询
  // 已发出、结果尚未落地之间时，除照常写入当前 state 外还记录到这里，
  // 供 reload 完成时的原子替换合并 —— 否则"用新页整表替换"会把并发增量
  // 清掉（旧实现先清空列表时此问题被掩盖）。
  const addedDuringReloadRef = useRef<MediaItemDto[]>([]);
  const removedDuringReloadRef = useRef<Set<string>>(new Set());

  const fetchPage = useCallback(async (pageCursor: MediaCursor, revision: number): Promise<MediaPageResponse | null> => {
    const response = await listMediaPage({
      cursor: pageCursor,
      pageSize: GALLERY_PAGE_SIZE,
      sort: 'dateDesc',
    });

    if (revision !== revisionRef.current) {
      // 已被更新的 reload/loadAll 取代：reload 落地时会整表重建列表与
      // seenMediaIds，此处写入会把过时的分页数据拼回新列表。
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
      // 复位会侵吞取代者的加载窗口（其结果落地前旧数据仍在屏，loading 掉
      // 线会让空态/加载态判断闪烁）。
      if (revision === revisionRef.current) {
        setIsLoading(false);
      }
    }
  }, [cursor, isLoading, fetchPage]);

  // SWR 式刷新：不清空任何列表状态（items/cursor/totalCount/seenMediaIds
  // 均保持），旧数据在首页返回前持续可见；新首页落地时一次性原子替换。
  // 旧实现先 setItems([]) 再 fetchPage 追加，刷新瞬间整个网格白屏回填。
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

    try {
      // 直连服务而非 fetchPage：fetchPage 会向 itemsRef 追加（分页语义），
      // 而这里要的是"整页替换"。查询在旧的列表状态下发出。
      const response = await listMediaPage({
        cursor: null,
        pageSize: GALLERY_PAGE_SIZE,
        sort: 'dateDesc',
      });

      if (revision !== revisionRef.current) {
        // 已被更新的 reload/loadAll 取代：丢弃本次结果。finally 仍会复位
        // inflight 与跟踪 refs —— 在飞期间的 addItems 自身已写入 state，
        // 不会丢数据。
        return;
      }

      // 原子替换，同时合入在飞窗口内的并发增量：
      // - removedDuringReload：窗口内 removeItems 标记的删除，从新页过滤，
      //   防止已删项被新页数据"复活"；
      // - addedDuringReload：窗口内 addItems 插入、且新页未包含的项
      //   （其媒体库扫描在本查询之后完成）——dateDesc 下它们是最新的，
      //   前插以对齐 addItems 的语义。
      const removed = removedDuringReloadRef.current;
      const pageItems = response.items.filter((i) => !removed.has(i.mediaId));
      const pageIds = new Set(pageItems.map((i) => i.mediaId));
      const preservedAdds = addedDuringReloadRef.current.filter(
        (i) => !pageIds.has(i.mediaId) && !removed.has(i.mediaId),
      );

      const nextItems = [...preservedAdds, ...pageItems];
      itemsRef.current = nextItems;
      setItems(nextItems);
      cursorRef.current = response.nextCursor;
      setCursor(response.nextCursor);
      setTotalCount(response.totalCount + preservedAdds.length);
      seenMediaIdsRef.current = new Set(nextItems.map((i) => i.mediaId));
    } catch (err) {
      // SWR：失败时旧数据保留在屏（不清空 items），仅记录错误。
      if (revision === revisionRef.current) {
        setError(err instanceof Error ? err.message : 'Failed to load page');
      }
    } finally {
      reloadInflightRef.current = false;
      // 跟踪 refs 无论成功/被取代一律复位：成功路径已在上面消费；被取代
      // 路径中 addItems 自身已把数据写入 live state，无需（也不得）再补。
      addedDuringReloadRef.current = [];
      removedDuringReloadRef.current = new Set();
      if (revision === revisionRef.current) {
        setIsLoading(false);
      }
    }
  }, []);

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

    // reload 在飞：额外记录删除 id，reload 完成时的原子替换会从新页里
    // 过滤掉它们，防止已删除项被即将落地的新首页复活。
    if (reloadInflightRef.current) {
      mediaIds.forEach((id) => removedDuringReloadRef.current.add(id));
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
      // reload 在飞：额外记录到合并缓冲 —— reload 完成时的原子替换若发现
      // 这些项不在新页里（其媒体库扫描在查询之后完成）则前插保留，防止
      // 整表替换把并发插入清掉。下方照常写入当前 state（立即上屏）。
      if (reloadInflightRef.current) {
        addedDuringReloadRef.current.push(...itemsToAdd);
      }
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
