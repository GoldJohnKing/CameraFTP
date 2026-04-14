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
});
