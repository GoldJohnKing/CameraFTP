/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { invoke } from '@tauri-apps/api/core';
import type { FileInfo } from '../types';
import { isGalleryMediaAvailable, listGalleryMedia } from './gallery-media';

export type LatestPhotoFile = Pick<FileInfo, 'filename' | 'path'>;

export async function fetchLatestPhotoFile(): Promise<LatestPhotoFile | null> {
  if (isGalleryMediaAvailable()) {
    const images = await listGalleryMedia();
    const latestImage = images[0] ?? null;
    return latestImage
      ? {
          filename: latestImage.filename,
          path: latestImage.path,
        }
      : null;
  }

  return invoke<FileInfo | null>('get_latest_image');
}
