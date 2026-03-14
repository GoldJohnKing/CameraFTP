/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { describe, it, expect } from 'vitest';
import { shouldRefreshOnEvent, toGalleryImage } from '../media-store-events';
import type { MediaStoreEntry } from '../media-store-events';

describe('media-store-events', () => {
  it('refreshes only on media-store-ready', () => {
  expect(shouldRefreshOnEvent('file-uploaded')).toBe(false);
  expect(shouldRefreshOnEvent('media-store-ready')).toBe(true);
});

  it('maps mediastore entry to gallery image', () => {
    const entry: MediaStoreEntry = { uri: 'content://media/1', displayName: 'IMG_1.JPG', dateModified: 1 };
    expect(toGalleryImage(entry).path).toBe(entry.uri);
  });
});
