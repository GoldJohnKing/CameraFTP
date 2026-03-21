/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { describe, expect, it, vi } from 'vitest';
import { isGalleryMediaAvailable, listGalleryMedia } from '../gallery-media';

describe('gallery-media service', () => {
  it('maps MediaStore entries to gallery images', async () => {
    const listMediaStoreImages = vi.fn().mockResolvedValue(JSON.stringify([
      { uri: 'content://a', displayName: 'a.jpg', dateModified: 123 },
    ]));

    window.GalleryAndroid = {
      listMediaStoreImages,
    } as unknown as typeof window.GalleryAndroid;

    await expect(listGalleryMedia()).resolves.toEqual([
      {
        path: 'content://a',
        filename: 'a.jpg',
        sortTime: 123,
      },
    ]);
  });

  it('reports availability based on GalleryAndroid bridge', () => {
    window.GalleryAndroid = undefined;
    expect(isGalleryMediaAvailable()).toBe(false);

    window.GalleryAndroid = {
      listMediaStoreImages: vi.fn(),
    } as unknown as typeof window.GalleryAndroid;
    expect(isGalleryMediaAvailable()).toBe(true);
  });
});
