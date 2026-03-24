/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { describe, expect, it, vi } from 'vitest';
import { isGalleryMediaAvailable } from '../gallery-media';

describe('gallery-media service', () => {
  it('reports availability based on GalleryAndroid bridge', () => {
    window.GalleryAndroid = undefined;
    expect(isGalleryMediaAvailable()).toBe(false);

    window.GalleryAndroid = {
      listMediaStoreImages: vi.fn(),
    } as unknown as typeof window.GalleryAndroid;
    expect(isGalleryMediaAvailable()).toBe(true);
  });
});
