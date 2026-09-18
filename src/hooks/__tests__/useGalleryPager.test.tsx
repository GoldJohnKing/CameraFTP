/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { act } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useGalleryPager } from '../useGalleryPager';
import { flush } from '../../test-utils/flush';
import { setupReactRoot } from '../../test-utils/react-root';
type UseGalleryPagerResult = ReturnType<typeof useGalleryPager>;
import type { MediaItemDto, MediaPageResponse } from '../../types';

const { listMediaPageMock } = vi.hoisted(() => ({
  listMediaPageMock: vi.fn(),
}));

vi.mock('../../services/gallery-media-v2', () => ({
  listMediaPage: listMediaPageMock,
  GALLERY_PAGE_SIZE: 120,
}));

let latestResult: UseGalleryPagerResult | null = null;

/** Options forwarded to the load-all button; set per-test before clicking. */
let loadAllOpts: { untilMs?: number; marginPages?: number } | undefined;

/** Epoch-ms for a Y/M/D at local noon (robust to DST shifts). */
function dayMs(y: number, m: number, d: number): number {
  return new Date(y, m - 1, d, 12, 0, 0).getTime();
}

function PagerHarness() {
  latestResult = useGalleryPager();
  return (
    <div>
      <span data-testid="count">{latestResult.items.length}</span>
      <span data-testid="loading">{latestResult.isLoading ? 'yes' : 'no'}</span>
      <span data-testid="error">{latestResult.error ?? ''}</span>
      <span data-testid="cursor">{latestResult.cursor ?? 'null'}</span>
      <span data-testid="total-count">{latestResult.totalCount}</span>
      <button onClick={() => void latestResult!.loadNextPage()} data-testid="load-next">
        load-next
      </button>
      <button onClick={() => void latestResult!.reload()} data-testid="reload">
        reload
      </button>
      <button onClick={() => void latestResult!.loadAll(loadAllOpts)} data-testid="load-all">
        load-all
      </button>
      <button
        onClick={() => latestResult!.removeItems(new Set(['media-2']))}
        data-testid="remove-media-2"
      >
        remove-media-2
      </button>
    </div>
  );
}

function makePage(
  items: MediaItemDto[],
  nextCursor: string | null,
  _revisionToken = 'rev-1',
  totalCount = 0,
): MediaPageResponse {
  return { items, nextCursor, revisionToken: _revisionToken, totalCount };
}

function makeItem(mediaId: string, dateModifiedMs = 1000): MediaItemDto {
  return {
    mediaId,
    uri: `file:///media/${mediaId}.jpg`,
    dateModifiedMs,
    width: 1920,
    height: 1080,
    mimeType: 'image/jpeg',
    displayName: null,
    filePath: null,
  };
}

async function clickLoadNext(getContainer: () => HTMLDivElement): Promise<void> {
  await act(async () => {
    getContainer().querySelector('[data-testid="load-next"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    await flush();
  });
}

describe('useGalleryPager', () => {
  const { getContainer, getRoot } = setupReactRoot();

  beforeEach(() => {
    listMediaPageMock.mockReset();
    latestResult = null;
    loadAllOpts = undefined;
  });

  async function renderHarness(): Promise<void> {
    await act(async () => {
      getRoot().render(<PagerHarness />);
      await flush();
    });
  }

  it('loads first page successfully', async () => {
    listMediaPageMock.mockResolvedValueOnce(
      makePage([makeItem('media-1'), makeItem('media-2')], 'cursor-1', 'rev-1'),
    );

    await renderHarness();
    await clickLoadNext(getContainer);

    expect(listMediaPageMock).toHaveBeenCalledTimes(1);
    expect(listMediaPageMock).toHaveBeenCalledWith({
      cursor: null,
      pageSize: 120,
      sort: 'dateDesc',
    });
    expect(getContainer().querySelector('[data-testid="count"]')?.textContent).toBe('2');
    expect(getContainer().querySelector('[data-testid="loading"]')?.textContent).toBe('no');
    expect(getContainer().querySelector('[data-testid="cursor"]')?.textContent).toBe('cursor-1');
    expect(getContainer().querySelector('[data-testid="error"]')?.textContent).toBe('');
  });

  it('appends items on subsequent loadNextPage calls', async () => {
    listMediaPageMock.mockResolvedValueOnce(
      makePage([makeItem('media-1')], 'cursor-1', 'rev-1'),
    );

    await renderHarness();
    await clickLoadNext(getContainer);

    expect(getContainer().querySelector('[data-testid="count"]')?.textContent).toBe('1');

    listMediaPageMock.mockResolvedValueOnce(
      makePage([makeItem('media-2')], 'cursor-2', 'rev-1'),
    );

    await clickLoadNext(getContainer);

    expect(listMediaPageMock).toHaveBeenCalledTimes(2);
    expect(listMediaPageMock).toHaveBeenLastCalledWith({
      cursor: 'cursor-1',
      pageSize: 120,
      sort: 'dateDesc',
    });
    expect(getContainer().querySelector('[data-testid="count"]')?.textContent).toBe('2');
    expect(getContainer().querySelector('[data-testid="cursor"]')?.textContent).toBe('cursor-2');
  });

  it('restarts from first page when stale_cursor returned', async () => {
    listMediaPageMock.mockResolvedValueOnce(
      makePage([makeItem('media-1')], 'cursor-1', 'rev-1'),
    );

    await renderHarness();
    await clickLoadNext(getContainer);

    expect(getContainer().querySelector('[data-testid="count"]')?.textContent).toBe('1');

    listMediaPageMock.mockRejectedValueOnce(new Error('stale_cursor'));
    listMediaPageMock.mockResolvedValueOnce(
      makePage([makeItem('media-1'), makeItem('media-3')], null, 'rev-2'),
    );

    await clickLoadNext(getContainer);

    expect(listMediaPageMock).toHaveBeenCalledTimes(3);
    expect(getContainer().querySelector('[data-testid="error"]')?.textContent).toBe('');
    expect(getContainer().querySelector('[data-testid="cursor"]')?.textContent).toBe('null');
    expect(getContainer().querySelector('[data-testid="count"]')?.textContent).toBe('1');
    expect(latestResult!.items.map((i) => i.mediaId)).toEqual(['media-3']);
  });

  it.each([
    { nextCursor: null, description: 'null cursor (end of data)' },
    { nextCursor: 'cursor-2', description: 'non-null cursor (more pages)' },
  ])('deduplicates by seenMediaIds during stale cursor rebuild ($description)', async ({ nextCursor }) => {
    listMediaPageMock.mockResolvedValueOnce(
      makePage([makeItem('media-1'), makeItem('media-2')], 'cursor-1', 'rev-1'),
    );

    await renderHarness();
    await clickLoadNext(getContainer);

    expect(getContainer().querySelector('[data-testid="count"]')?.textContent).toBe('2');

    listMediaPageMock.mockRejectedValueOnce(new Error('stale_cursor'));
    listMediaPageMock.mockResolvedValueOnce(
      makePage(
        [makeItem('media-1'), makeItem('media-2'), makeItem('media-3')],
        nextCursor,
        'rev-2',
      ),
    );

    await clickLoadNext(getContainer);

    expect(listMediaPageMock).toHaveBeenCalledTimes(3);
    expect(getContainer().querySelector('[data-testid="count"]')?.textContent).toBe('1');
    expect(latestResult!.items.map((i) => i.mediaId)).toEqual(['media-3']);
    expect(getContainer().querySelector('[data-testid="error"]')?.textContent).toBe('');
    expect(getContainer().querySelector('[data-testid="cursor"]')?.textContent).toBe(nextCursor ?? 'null');
  });

  it('resets everything on reload', async () => {
    listMediaPageMock.mockResolvedValueOnce(
      makePage([makeItem('media-1')], 'cursor-1', 'rev-1'),
    );

    await renderHarness();
    await clickLoadNext(getContainer);

    expect(getContainer().querySelector('[data-testid="count"]')?.textContent).toBe('1');

    listMediaPageMock.mockResolvedValueOnce(
      makePage([makeItem('media-10'), makeItem('media-11')], 'cursor-new', 'rev-2'),
    );

    await act(async () => {
      getContainer().querySelector('[data-testid="reload"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    expect(listMediaPageMock).toHaveBeenCalledTimes(2);
    expect(listMediaPageMock).toHaveBeenLastCalledWith({
      cursor: null,
      pageSize: 120,
      sort: 'dateDesc',
    });
    expect(getContainer().querySelector('[data-testid="count"]')?.textContent).toBe('2');
    expect(getContainer().querySelector('[data-testid="cursor"]')?.textContent).toBe('cursor-new');
  });

  it('removes items by mediaId', async () => {
    listMediaPageMock.mockResolvedValueOnce(
      makePage([makeItem('media-1'), makeItem('media-2'), makeItem('media-3')], null, 'rev-1', 3),
    );

    await renderHarness();
    await clickLoadNext(getContainer);

    expect(getContainer().querySelector('[data-testid="count"]')?.textContent).toBe('3');
    expect(getContainer().querySelector('[data-testid="total-count"]')?.textContent).toBe('3');

    await act(async () => {
      getContainer().querySelector('[data-testid="remove-media-2"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    expect(getContainer().querySelector('[data-testid="count"]')?.textContent).toBe('2');
    expect(getContainer().querySelector('[data-testid="total-count"]')?.textContent).toBe('2');
    expect(latestResult!.items.map((i) => i.mediaId)).toEqual(['media-1', 'media-3']);
  });

  it('does not call listMediaPage when cursor is exhausted but items exist', async () => {
    // 游标早退：cursor=null 且已有数据（分页到底）时 loadNextPage 不应再
    // 发起 bridge 请求 —— 否则已耗尽游标会被当作"首载"，被响应中的第一页
    // nextCursor 重新续流。
    listMediaPageMock.mockResolvedValueOnce(
      makePage([makeItem('media-1'), makeItem('media-2')], null, 'rev-1', 2),
    );

    await renderHarness();
    await clickLoadNext(getContainer);

    expect(listMediaPageMock).toHaveBeenCalledTimes(1);
    expect(getContainer().querySelector('[data-testid="cursor"]')?.textContent).toBe('null');
    expect(getContainer().querySelector('[data-testid="count"]')?.textContent).toBe('2');

    await clickLoadNext(getContainer);
    await clickLoadNext(getContainer);

    expect(listMediaPageMock).toHaveBeenCalledTimes(1);
  });

  it('does not decrement totalCount below zero when removing extra ids', async () => {
    listMediaPageMock.mockResolvedValueOnce(
      makePage([makeItem('media-1')], null, 'rev-1', 1),
    );

    await renderHarness();
    await clickLoadNext(getContainer);

    expect(getContainer().querySelector('[data-testid="count"]')?.textContent).toBe('1');
    expect(getContainer().querySelector('[data-testid="total-count"]')?.textContent).toBe('1');

    await act(async () => {
      latestResult!.removeItems(new Set(['media-1', 'missing-media-id']));
      await flush();
    });

    expect(getContainer().querySelector('[data-testid="count"]')?.textContent).toBe('0');
    expect(getContainer().querySelector('[data-testid="total-count"]')?.textContent).toBe('0');
  });

  it('sets error on non-stale-cursor failure', async () => {
    listMediaPageMock.mockRejectedValueOnce(new Error('Network timeout'));

    await renderHarness();
    await clickLoadNext(getContainer);

    expect(getContainer().querySelector('[data-testid="error"]')?.textContent).toBe('Network timeout');
    expect(getContainer().querySelector('[data-testid="count"]')?.textContent).toBe('0');
  });

  it('prevents concurrent loadNextPage calls', async () => {
    let resolveFirst!: (value: MediaPageResponse) => void;
    const firstPromise = new Promise<MediaPageResponse>((res) => {
      resolveFirst = res;
    });
    listMediaPageMock.mockReturnValueOnce(firstPromise);

    await renderHarness();

    await act(async () => {
      getContainer().querySelector('[data-testid="load-next"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    expect(getContainer().querySelector('[data-testid="loading"]')?.textContent).toBe('yes');

    await act(async () => {
      getContainer().querySelector('[data-testid="load-next"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    expect(listMediaPageMock).toHaveBeenCalledTimes(1);

    await act(async () => {
      resolveFirst(makePage([makeItem('media-1')], 'cursor-1', 'rev-1'));
      await flush();
    });

    expect(getContainer().querySelector('[data-testid="loading"]')?.textContent).toBe('no');
    expect(getContainer().querySelector('[data-testid="count"]')?.textContent).toBe('1');
  });

  it('returns a stable object reference across rerenders without new data', async () => {
    listMediaPageMock.mockResolvedValueOnce(
      makePage([makeItem('media-1')], null, 'rev-1'),
    );

    await renderHarness();
    await clickLoadNext(getContainer);

    const first = latestResult!;

    await act(async () => {
      getRoot().render(<PagerHarness />);
      await flush();
    });

    expect(latestResult).toBe(first);
  });

  it('reload supersedes an in-flight loadNextPage and discards its stale page result', async () => {
    // 取代语义：翻页在飞（mock 挂起）时调用 reload 必须执行（不得被分页的
    // in-flight 吞掉）；resolve 后旧分页结果按代际检查丢弃，state 只保留
    // 新刷数据。
    let resolveLoadNext!: (value: MediaPageResponse) => void;
    const loadNextPromise = new Promise<MediaPageResponse>((res) => {
      resolveLoadNext = res;
    });
    listMediaPageMock.mockReturnValueOnce(loadNextPromise);

    await renderHarness();

    await act(async () => {
      getContainer().querySelector('[data-testid="load-next"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });
    expect(getContainer().querySelector('[data-testid="loading"]')?.textContent).toBe('yes');

    listMediaPageMock.mockResolvedValueOnce(
      makePage([makeItem('media-r1'), makeItem('media-r2')], 'cursor-r1', 'rev-2'),
    );

    await act(async () => {
      getContainer().querySelector('[data-testid="reload"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
      await flush();
    });

    expect(getContainer().querySelector('[data-testid="count"]')?.textContent).toBe('2');
    expect(getContainer().querySelector('[data-testid="cursor"]')?.textContent).toBe('cursor-r1');

    // 过时的翻页请求此刻才 resolve —— 不得写入 state。
    await act(async () => {
      resolveLoadNext(makePage([makeItem('media-old-1'), makeItem('media-old-2')], 'cursor-old', 'rev-1'));
      await flush();
      await flush();
    });

    expect(latestResult!.items.map((i) => i.mediaId)).toEqual(['media-r1', 'media-r2']);
    expect(getContainer().querySelector('[data-testid="cursor"]')?.textContent).toBe('cursor-r1');
    expect(getContainer().querySelector('[data-testid="loading"]')?.textContent).toBe('no');
  });

  it('loadAll proceeds while loadNextPage is in flight and completes without loss', async () => {
    // 取代语义：loadAll 不被在飞分页吞掉，且完成后全量数据完整；
    // 旧分页结果 resolve 后被丢弃。
    let resolveLoadNext!: (value: MediaPageResponse) => void;
    const loadNextPromise = new Promise<MediaPageResponse>((res) => {
      resolveLoadNext = res;
    });
    listMediaPageMock.mockReturnValueOnce(loadNextPromise);

    await renderHarness();

    await act(async () => {
      getContainer().querySelector('[data-testid="load-next"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });
    expect(getContainer().querySelector('[data-testid="loading"]')?.textContent).toBe('yes');

    // loadAll 取代在飞的翻页：拉全剩余页直到游标耗尽。
    listMediaPageMock.mockResolvedValueOnce(
      makePage([makeItem('media-a1')], 'cursor-a1', 'rev-2'),
    );
    listMediaPageMock.mockResolvedValueOnce(
      makePage([makeItem('media-a2')], null, 'rev-2'),
    );

    await act(async () => {
      getContainer().querySelector('[data-testid="load-all"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
      await flush();
      await flush();
      await flush();
    });

    expect(getContainer().querySelector('[data-testid="count"]')?.textContent).toBe('2');

    // 过时的翻页请求 resolve 后被丢弃，全量结果不丢。
    await act(async () => {
      resolveLoadNext(makePage([makeItem('media-old')], 'cursor-old', 'rev-1'));
      await flush();
      await flush();
    });

    expect(latestResult!.items.map((i) => i.mediaId)).toEqual(['media-a1', 'media-a2']);
    expect(getContainer().querySelector('[data-testid="cursor"]')?.textContent).toBe('null');
    expect(getContainer().querySelector('[data-testid="loading"]')?.textContent).toBe('no');
  });

  it('loadAll without untilMs still loads to cursor exhaustion', async () => {
    // 无界模式（日期列表构建）：不传 untilMs 时保持拉到底 —— 有界化不得
    // 改变现有"完整日期列表"路径的语义。
    listMediaPageMock
      .mockResolvedValueOnce(makePage([makeItem('media-1')], 'cursor-1', 'rev-1'))
      .mockResolvedValueOnce(makePage([makeItem('media-2')], null, 'rev-1'));

    await renderHarness();

    await act(async () => {
      getContainer().querySelector('[data-testid="load-all"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
      await flush();
      await flush();
      await flush();
    });

    expect(listMediaPageMock).toHaveBeenCalledTimes(2);
    expect(getContainer().querySelector('[data-testid="cursor"]')?.textContent).toBe('null');
    expect(getContainer().querySelector('[data-testid="count"]')?.textContent).toBe('2');
  });

  it('loadAll stops two margin pages past the page covering the target day', async () => {
    // 有界模式：目标日 D2 的条目横跨 cursor-1 之后的第一页；覆盖判定需要
    // 拉到一条"严格更早日"的 item（D3 页），随后仅再拉 2 页余量即停，
    // 游标不耗尽 —— 不再拉完整个媒体库。
    const D1 = dayMs(2026, 7, 19);
    const D2 = dayMs(2026, 7, 18);
    const D3 = dayMs(2026, 7, 17);
    listMediaPageMock
      .mockResolvedValueOnce(makePage([makeItem('p1-a', D1)], 'cursor-1', 'rev-1'))
      .mockResolvedValueOnce(makePage([makeItem('p2-a', D2), makeItem('p2-b', D2)], 'cursor-2', 'rev-1'))
      .mockResolvedValueOnce(makePage([makeItem('p3-a', D3)], 'cursor-3', 'rev-1'))
      .mockResolvedValueOnce(makePage([makeItem('p4-a', D3)], 'cursor-4', 'rev-1'))
      .mockResolvedValueOnce(makePage([makeItem('p5-a', D3)], 'cursor-5', 'rev-1'));

    await renderHarness();
    loadAllOpts = { untilMs: D2 };

    await act(async () => {
      getContainer().querySelector('[data-testid="load-all"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      for (let i = 0; i < 12; i++) await flush();
    });

    // p1 (D1, 未覆盖) → p2 (D2, 同日不算覆盖) → p3 (D3 < D2，覆盖页) →
    // p4、p5（余量 2 页）→ 停止。第 6 页 mock 未消费。
    expect(listMediaPageMock).toHaveBeenCalledTimes(5);
    expect(getContainer().querySelector('[data-testid="cursor"]')?.textContent).toBe('cursor-5');
    expect(getContainer().querySelector('[data-testid="count"]')?.textContent).toBe('6');
    expect(getContainer().querySelector('[data-testid="loading"]')?.textContent).toBe('no');

    // 向下无限滚动不受影响：loadNextPage 从保留的 cursor-5 继续追加。
    listMediaPageMock.mockResolvedValueOnce(
      makePage([makeItem('p6-a', D3)], null, 'rev-1'),
    );
    await clickLoadNext(getContainer);

    expect(listMediaPageMock).toHaveBeenLastCalledWith({
      cursor: 'cursor-5',
      pageSize: 120,
      sort: 'dateDesc',
    });
    expect(getContainer().querySelector('[data-testid="count"]')?.textContent).toBe('7');
  });

  it('loadAll honors a custom marginPages of zero', async () => {
    const D1 = dayMs(2026, 7, 19);
    const D2 = dayMs(2026, 7, 18);
    const D3 = dayMs(2026, 7, 17);
    listMediaPageMock
      .mockResolvedValueOnce(makePage([makeItem('p1-a', D1)], 'cursor-1', 'rev-1'))
      .mockResolvedValueOnce(makePage([makeItem('p2-a', D2)], 'cursor-2', 'rev-1'))
      .mockResolvedValueOnce(makePage([makeItem('p3-a', D3)], 'cursor-3', 'rev-1'));

    await renderHarness();
    loadAllOpts = { untilMs: D2, marginPages: 0 };

    await act(async () => {
      getContainer().querySelector('[data-testid="load-all"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      for (let i = 0; i < 8; i++) await flush();
    });

    // 覆盖页（p3）即停，不拉余量。
    expect(listMediaPageMock).toHaveBeenCalledTimes(3);
    expect(getContainer().querySelector('[data-testid="cursor"]')?.textContent).toBe('cursor-3');
  });

  it('loadAll loads to exhaustion when the target day precedes all data', async () => {
    const D1 = dayMs(2026, 7, 19);
    const D3 = dayMs(2026, 7, 17);
    const ANCIENT = dayMs(2020, 1, 1);
    listMediaPageMock
      .mockResolvedValueOnce(makePage([makeItem('p1-a', D1)], 'cursor-1', 'rev-1'))
      .mockResolvedValueOnce(makePage([makeItem('p2-a', D3)], null, 'rev-1'));

    await renderHarness();
    loadAllOpts = { untilMs: ANCIENT };

    await act(async () => {
      getContainer().querySelector('[data-testid="load-all"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      for (let i = 0; i < 8; i++) await flush();
    });

    // 目标日早于全部数据：覆盖只能由游标耗尽达成 —— 等价于全量，正确终止。
    expect(listMediaPageMock).toHaveBeenCalledTimes(2);
    expect(getContainer().querySelector('[data-testid="cursor"]')?.textContent).toBe('null');
    expect(getContainer().querySelector('[data-testid="count"]')?.textContent).toBe('2');
  });

  it('loadAll fetches only margin pages when the loaded range already covers the target day', async () => {
    const D1 = dayMs(2026, 7, 19);
    const D2 = dayMs(2026, 7, 18);
    const D3 = dayMs(2026, 7, 17);
    // 预载一页：同时含 D2 与更早的 D3 → 目标日 D2 已被覆盖。
    listMediaPageMock.mockResolvedValueOnce(
      makePage([makeItem('p1-a', D1), makeItem('p1-b', D2), makeItem('p1-c', D3)], 'cursor-1', 'rev-1'),
    );

    await renderHarness();
    await clickLoadNext(getContainer);
    expect(listMediaPageMock).toHaveBeenCalledTimes(1);

    listMediaPageMock
      .mockResolvedValueOnce(makePage([makeItem('p2-a', D3)], 'cursor-2', 'rev-1'))
      .mockResolvedValueOnce(makePage([makeItem('p3-a', D3)], 'cursor-3', 'rev-1'));

    loadAllOpts = { untilMs: D2 };
    await act(async () => {
      getContainer().querySelector('[data-testid="load-all"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      for (let i = 0; i < 8; i++) await flush();
    });

    // 已覆盖：每页都计为余量页 → 只再拉 2 页即停。
    expect(listMediaPageMock).toHaveBeenCalledTimes(3);
    expect(getContainer().querySelector('[data-testid="cursor"]')?.textContent).toBe('cursor-3');
    expect(getContainer().querySelector('[data-testid="count"]')?.textContent).toBe('5');
  });
});
