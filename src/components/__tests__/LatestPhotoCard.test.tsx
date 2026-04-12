/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { act, type ReactNode } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { LatestPhotoCard } from '../LatestPhotoCard';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

const openPreviewMock = vi.fn();

vi.mock('../../hooks/useImagePreviewOpener', () => ({
  useImagePreviewOpener: () => openPreviewMock,
}));

vi.mock('../../stores/serverStore', () => ({
  useServerStore: () => ({
    stats: {
      lastFile: null,
    },
  }),
}));

vi.mock('../ui', () => ({
  IconContainer: ({ children }: { children: ReactNode }) => <div>{children}</div>,
}));

const listMediaPageMock = vi.fn();

const galleryAndroidV2 = {
  listMediaPage: listMediaPageMock,
} as Pick<NonNullable<Window['GalleryAndroidV2']>, 'listMediaPage'>;

import { flush } from '../../test-utils/flush';

describe('LatestPhotoCard', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(async () => {
    vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    window.GalleryAndroidV2 = galleryAndroidV2 as Window['GalleryAndroidV2'];
    listMediaPageMock.mockReset();
    openPreviewMock.mockReset();
    listMediaPageMock.mockResolvedValue(JSON.stringify({
      items: [
        {
          mediaId: '2',
          uri: 'content://media/2/fresh.jpg',
          dateModifiedMs: 200,
          width: 1920,
          height: 1080,
          mimeType: 'image/jpeg',
        },
      ],
      nextCursor: null,
      revisionToken: 'tok',
    }));
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    delete window.GalleryAndroidV2;
    vi.unstubAllGlobals();
  });

  it('updates latest photo when a gallery refresh is requested', async () => {
    await act(async () => {
      root.render(<LatestPhotoCard />);
      await flush();
    });

    await act(async () => {
      window.dispatchEvent(new CustomEvent('latest-photo-refresh-requested', {
        detail: { reason: 'manual' },
      }));
      await flush();
    });

    expect(listMediaPageMock).toHaveBeenCalled();
    expect(container.textContent).toContain('fresh.jpg');
  });

  it('passes a Gallery V2 URI provider when opening latest photo preview', async () => {
    await act(async () => {
      root.render(<LatestPhotoCard />);
      await flush();
    });

    await act(async () => {
      container.querySelector('button')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    expect(openPreviewMock).toHaveBeenCalledTimes(1);
    expect(openPreviewMock).toHaveBeenCalledWith(
      expect.objectContaining({
        filePath: 'content://media/2/fresh.jpg',
        getAllUris: expect.any(Function),
      }),
    );

    const params = openPreviewMock.mock.calls[0][0] as {
      getAllUris: () => Promise<string[]>;
    };

    listMediaPageMock.mockResolvedValueOnce(JSON.stringify({
      items: [
        {
          mediaId: '2',
          uri: 'content://media/2/fresh.jpg',
          dateModifiedMs: 200,
          width: 1920,
          height: 1080,
          mimeType: 'image/jpeg',
        },
        {
          mediaId: '1',
          uri: 'content://media/1/older.jpg',
          dateModifiedMs: 100,
          width: 1920,
          height: 1080,
          mimeType: 'image/jpeg',
        },
      ],
      nextCursor: null,
      revisionToken: 'tok-2',
      totalCount: 2,
    }));

    await expect(params.getAllUris()).resolves.toEqual([
      'content://media/2/fresh.jpg',
      'content://media/1/older.jpg',
    ]);
    expect(listMediaPageMock).toHaveBeenLastCalledWith(JSON.stringify({
      cursor: null,
      pageSize: 120,
      sort: 'dateDesc',
    }));
  });
});
