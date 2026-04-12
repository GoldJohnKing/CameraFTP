/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import type { MediaItemDto } from '../types';

/**
 * Creates an array of mock MediaItemDto objects for testing.
 */
export function makeItems(count: number): MediaItemDto[] {
  return Array.from({ length: count }, (_, i) => ({
    mediaId: `media-${i}`,
    uri: `content://media/${i}`,
    dateModifiedMs: 1000 + i,
    width: 100,
    height: 100,
    mimeType: 'image/jpeg',
    displayName: null,
  }));
}
