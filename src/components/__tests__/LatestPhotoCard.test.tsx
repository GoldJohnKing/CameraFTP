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

async function flush(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}

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
});
