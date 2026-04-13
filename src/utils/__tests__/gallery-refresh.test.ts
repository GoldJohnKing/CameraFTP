/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { describe, expect, it, vi } from 'vitest';
import {
  GALLERY_REFRESH_REQUESTED_EVENT,
  LATEST_PHOTO_REFRESH_REQUESTED_EVENT,
  requestMediaLibraryRefresh,
} from '../gallery-refresh';

describe('gallery-refresh', () => {
  it.each([
    { name: 'manual refresh', options: { reason: 'manual' as const }, expected: { reason: 'manual' as const } },
    { name: 'delete refresh', options: { reason: 'delete' as const, timestamp: 123 }, expected: { reason: 'delete' as const, timestamp: 123 } },
  ])('dispatches events for $name', ({ options, expected }) => {
    const galleryHandler = vi.fn();
    const latestHandler = vi.fn();

    window.addEventListener(GALLERY_REFRESH_REQUESTED_EVENT, galleryHandler);
    window.addEventListener(LATEST_PHOTO_REFRESH_REQUESTED_EVENT, latestHandler);

    requestMediaLibraryRefresh(options);

    expect(galleryHandler).toHaveBeenCalledTimes(1);
    expect(latestHandler).toHaveBeenCalledTimes(1);
    expect(galleryHandler.mock.calls[0]?.[0]).toMatchObject({ detail: expected });
    expect(latestHandler.mock.calls[0]?.[0]).toMatchObject({ detail: expected });

    window.removeEventListener(GALLERY_REFRESH_REQUESTED_EVENT, galleryHandler);
    window.removeEventListener(LATEST_PHOTO_REFRESH_REQUESTED_EVENT, latestHandler);
  });

});
