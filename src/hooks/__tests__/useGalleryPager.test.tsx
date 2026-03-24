/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useGalleryPager, type UseGalleryPagerResult } from '../useGalleryPager';
import type { MediaItemDto, MediaPageResponse } from '../../types/gallery-v2';

const { listMediaPageMock } = vi.hoisted(() => ({
  listMediaPageMock: vi.fn(),
}));

vi.mock('../../services/gallery-media-v2', () => ({
  listMediaPage: listMediaPageMock,
}));

let latestResult: UseGalleryPagerResult | null = null;

function PagerHarness() {
  latestResult = useGalleryPager();
  return (
    <div>
      <span data-testid="count">{latestResult.items.length}</span>
      <span data-testid="loading">{latestResult.isLoading ? 'yes' : 'no'}</span>
      <span data-testid="error">{latestResult.error ?? ''}</span>
      <span data-testid="cursor">{latestResult.cursor ?? 'null'}</span>
      <span data-testid="revision">{latestResult.revisionToken}</span>
      <button onClick={() => void latestResult!.loadNextPage()} data-testid="load-next">
        load-next
      </button>
      <button onClick={() => void latestResult!.reload()} data-testid="reload">
        reload
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

async function flush(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}

function makePage(
  items: MediaItemDto[],
  nextCursor: string | null,
  revisionToken = 'rev-1',
): MediaPageResponse {
  return { items, nextCursor, revisionToken };
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
  };
}

async function clickLoadNext(): Promise<void> {
  await act(async () => {
    container.querySelector('[data-testid="load-next"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    await flush();
  });
}

let container: HTMLDivElement;

describe('useGalleryPager', () => {
  let root: Root;

  beforeEach(() => {
    vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);

    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);

    listMediaPageMock.mockReset();
    latestResult = null;
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    vi.unstubAllGlobals();
  });

  async function renderHarness(): Promise<void> {
    await act(async () => {
      root.render(<PagerHarness />);
      await flush();
    });
  }

  it('loads first page successfully', async () => {
    listMediaPageMock.mockResolvedValueOnce(
      makePage([makeItem('media-1'), makeItem('media-2')], 'cursor-1', 'rev-1'),
    );

    await renderHarness();
    await clickLoadNext();

    expect(listMediaPageMock).toHaveBeenCalledTimes(1);
    expect(listMediaPageMock).toHaveBeenCalledWith({
      cursor: null,
      pageSize: 120,
      sort: 'dateDesc',
    });
    expect(container.querySelector('[data-testid="count"]')?.textContent).toBe('2');
    expect(container.querySelector('[data-testid="loading"]')?.textContent).toBe('no');
    expect(container.querySelector('[data-testid="cursor"]')?.textContent).toBe('cursor-1');
    expect(container.querySelector('[data-testid="revision"]')?.textContent).toBe('rev-1');
    expect(container.querySelector('[data-testid="error"]')?.textContent).toBe('');
  });

  it('appends items on subsequent loadNextPage calls', async () => {
    listMediaPageMock.mockResolvedValueOnce(
      makePage([makeItem('media-1')], 'cursor-1', 'rev-1'),
    );

    await renderHarness();
    await clickLoadNext();

    expect(container.querySelector('[data-testid="count"]')?.textContent).toBe('1');

    listMediaPageMock.mockResolvedValueOnce(
      makePage([makeItem('media-2')], 'cursor-2', 'rev-1'),
    );

    await clickLoadNext();

    expect(listMediaPageMock).toHaveBeenCalledTimes(2);
    expect(listMediaPageMock).toHaveBeenLastCalledWith({
      cursor: 'cursor-1',
      pageSize: 120,
      sort: 'dateDesc',
    });
    expect(container.querySelector('[data-testid="count"]')?.textContent).toBe('2');
    expect(container.querySelector('[data-testid="cursor"]')?.textContent).toBe('cursor-2');
  });

  it('restarts from first page when stale_cursor returned', async () => {
    listMediaPageMock.mockResolvedValueOnce(
      makePage([makeItem('media-1')], 'cursor-1', 'rev-1'),
    );

    await renderHarness();
    await clickLoadNext();

    expect(container.querySelector('[data-testid="count"]')?.textContent).toBe('1');

    listMediaPageMock.mockRejectedValueOnce(new Error('stale_cursor'));
    listMediaPageMock.mockResolvedValueOnce(
      makePage([makeItem('media-1'), makeItem('media-3')], null, 'rev-2'),
    );

    await clickLoadNext();

    expect(listMediaPageMock).toHaveBeenCalledTimes(3);
    expect(container.querySelector('[data-testid="error"]')?.textContent).toBe('');
    expect(container.querySelector('[data-testid="cursor"]')?.textContent).toBe('null');
    expect(container.querySelector('[data-testid="count"]')?.textContent).toBe('1');
    expect(latestResult!.items.map((i) => i.mediaId)).toEqual(['media-3']);
    expect(container.querySelector('[data-testid="revision"]')?.textContent).toBe('rev-2');
  });

  it('deduplicates by seenMediaIds during stale cursor rebuild', async () => {
    listMediaPageMock.mockResolvedValueOnce(
      makePage([makeItem('media-1'), makeItem('media-2')], 'cursor-1', 'rev-1'),
    );

    await renderHarness();
    await clickLoadNext();

    expect(container.querySelector('[data-testid="count"]')?.textContent).toBe('2');

    listMediaPageMock.mockRejectedValueOnce(new Error('stale_cursor'));
    listMediaPageMock.mockResolvedValueOnce(
      makePage(
        [makeItem('media-1'), makeItem('media-2'), makeItem('media-3')],
        null,
        'rev-2',
      ),
    );

    await clickLoadNext();

    expect(listMediaPageMock).toHaveBeenCalledTimes(3);
    expect(container.querySelector('[data-testid="count"]')?.textContent).toBe('1');
    expect(latestResult!.items.map((i) => i.mediaId)).toEqual(['media-3']);
    expect(container.querySelector('[data-testid="error"]')?.textContent).toBe('');
  });

  it('deduplicates items when stale_cursor rebuild returns overlapping data', async () => {
    listMediaPageMock.mockResolvedValueOnce(
      makePage([makeItem('media-1'), makeItem('media-2')], 'cursor-1', 'rev-1'),
    );

    await renderHarness();
    await clickLoadNext();

    expect(container.querySelector('[data-testid="count"]')?.textContent).toBe('2');

    listMediaPageMock.mockRejectedValueOnce(new Error('stale_cursor'));
    listMediaPageMock.mockResolvedValueOnce(
      makePage(
        [makeItem('media-1'), makeItem('media-2'), makeItem('media-3')],
        'cursor-2',
        'rev-2',
      ),
    );

    await clickLoadNext();

    expect(listMediaPageMock).toHaveBeenCalledTimes(3);
    expect(container.querySelector('[data-testid="count"]')?.textContent).toBe('1');
    expect(latestResult!.items.map((i) => i.mediaId)).toEqual(['media-3']);
    expect(container.querySelector('[data-testid="error"]')?.textContent).toBe('');
    expect(container.querySelector('[data-testid="revision"]')?.textContent).toBe('rev-2');
    expect(container.querySelector('[data-testid="cursor"]')?.textContent).toBe('cursor-2');
  });

  it('resets everything on reload', async () => {
    listMediaPageMock.mockResolvedValueOnce(
      makePage([makeItem('media-1')], 'cursor-1', 'rev-1'),
    );

    await renderHarness();
    await clickLoadNext();

    expect(container.querySelector('[data-testid="count"]')?.textContent).toBe('1');

    listMediaPageMock.mockResolvedValueOnce(
      makePage([makeItem('media-10'), makeItem('media-11')], 'cursor-new', 'rev-2'),
    );

    await act(async () => {
      container.querySelector('[data-testid="reload"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    expect(listMediaPageMock).toHaveBeenCalledTimes(2);
    expect(listMediaPageMock).toHaveBeenLastCalledWith({
      cursor: null,
      pageSize: 120,
      sort: 'dateDesc',
    });
    expect(container.querySelector('[data-testid="count"]')?.textContent).toBe('2');
    expect(container.querySelector('[data-testid="cursor"]')?.textContent).toBe('cursor-new');
    expect(container.querySelector('[data-testid="revision"]')?.textContent).toBe('rev-2');
  });

  it('removes items by mediaId', async () => {
    listMediaPageMock.mockResolvedValueOnce(
      makePage([makeItem('media-1'), makeItem('media-2'), makeItem('media-3')], null, 'rev-1'),
    );

    await renderHarness();
    await clickLoadNext();

    expect(container.querySelector('[data-testid="count"]')?.textContent).toBe('3');

    await act(async () => {
      container.querySelector('[data-testid="remove-media-2"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    expect(container.querySelector('[data-testid="count"]')?.textContent).toBe('2');
    expect(latestResult!.items.map((i) => i.mediaId)).toEqual(['media-1', 'media-3']);
  });

  it('sets error on non-stale-cursor failure', async () => {
    listMediaPageMock.mockRejectedValueOnce(new Error('Network timeout'));

    await renderHarness();
    await clickLoadNext();

    expect(container.querySelector('[data-testid="error"]')?.textContent).toBe('Network timeout');
    expect(container.querySelector('[data-testid="count"]')?.textContent).toBe('0');
  });

  it('prevents concurrent loadNextPage calls', async () => {
    let resolveFirst!: (value: MediaPageResponse) => void;
    const firstPromise = new Promise<MediaPageResponse>((res) => {
      resolveFirst = res;
    });
    listMediaPageMock.mockReturnValueOnce(firstPromise);

    await renderHarness();

    await act(async () => {
      container.querySelector('[data-testid="load-next"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    expect(container.querySelector('[data-testid="loading"]')?.textContent).toBe('yes');

    await act(async () => {
      container.querySelector('[data-testid="load-next"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    expect(listMediaPageMock).toHaveBeenCalledTimes(1);

    await act(async () => {
      resolveFirst(makePage([makeItem('media-1')], 'cursor-1', 'rev-1'));
      await flush();
    });

    expect(container.querySelector('[data-testid="loading"]')?.textContent).toBe('no');
    expect(container.querySelector('[data-testid="count"]')?.textContent).toBe('1');
  });
});
