/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { toGalleryImage, type MediaStoreEntry } from '../utils/media-store-events';
import type { GalleryImage } from '../types';

export function isGalleryMediaAvailable(): boolean {
  return typeof window !== 'undefined' && !!window.GalleryAndroid;
}

export async function listGalleryMedia(): Promise<GalleryImage[]> {
  if (!window.GalleryAndroid) {
    return [];
  }

  const listJson = await window.GalleryAndroid.listMediaStoreImages();
  const entries = JSON.parse(listJson ?? '[]') as MediaStoreEntry[];
  return entries.map(toGalleryImage);
}
