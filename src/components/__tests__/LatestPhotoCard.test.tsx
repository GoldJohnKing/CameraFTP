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

const listMediaStoreImagesMock = vi.fn();

const galleryAndroid = {
  listMediaStoreImages: listMediaStoreImagesMock,
} as Pick<NonNullable<Window['GalleryAndroid']>, 'listMediaStoreImages'>;

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
    window.GalleryAndroid = galleryAndroid as Window['GalleryAndroid'];
    listMediaStoreImagesMock.mockReset();
    listMediaStoreImagesMock.mockResolvedValue(JSON.stringify([
      {
        uri: 'content://media/2',
        displayName: 'fresh.jpg',
        dateModified: 200,
      },
    ]));
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    delete window.GalleryAndroid;
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

    expect(listMediaStoreImagesMock).toHaveBeenCalled();
    expect(container.textContent).toContain('fresh.jpg');
  });
});
