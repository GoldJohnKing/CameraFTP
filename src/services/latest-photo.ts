/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { invoke } from '@tauri-apps/api/core';
import type { FileInfo } from '../types';
import { isGalleryV2Available, listMediaPageV2 } from './gallery-media-v2';

export type LatestPhotoFile = Pick<FileInfo, 'filename' | 'path'>;

export async function fetchLatestPhotoFile(): Promise<LatestPhotoFile | null> {
  if (isGalleryV2Available()) {
    const page = await listMediaPageV2({ cursor: null, pageSize: 1, sort: 'dateDesc' });
    const latest = page.items[0] ?? null;
    return latest
      ? {
          filename: latest.displayName ?? latest.uri.split('/').pop() ?? latest.mediaId,
          path: latest.uri,
        }
      : null;
  }

  return invoke<FileInfo | null>('get_latest_image');
}
