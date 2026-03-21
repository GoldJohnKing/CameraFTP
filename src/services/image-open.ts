/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { invoke } from '@tauri-apps/api/core';
import type { ExifInfo } from '../types';
import type { MediaStoreEntry } from '../utils/media-store-events';

interface OpenImagePreviewParams {
  filePath: string;
  openMethod?: string;
  allUris?: string[];
  getAllUris?: () => Promise<string[]>;
}

async function getMediaStoreUris(): Promise<string[]> {
  if (!window.GalleryAndroid) {
    return [];
  }

  const listJson = await window.GalleryAndroid.listMediaStoreImages();
  const entries = JSON.parse(listJson ?? '[]') as MediaStoreEntry[];
  return entries.map((entry) => entry.uri);
}

async function sendExifToViewer(path: string): Promise<void> {
  if (!window.ImageViewerAndroid?.onExifResult) {
    return;
  }

  const realPath = window.ImageViewerAndroid.resolveFilePath?.(path) ?? path;
  try {
    const exif = await invoke<ExifInfo | null>('get_image_exif', { filePath: realPath });
    window.ImageViewerAndroid.onExifResult(exif ? JSON.stringify(exif) : null);
  } catch {
    // Keep current behavior: ignore EXIF fetch failures silently.
  }
}

export async function openImagePreview({
  filePath,
  openMethod,
  allUris,
  getAllUris,
}: OpenImagePreviewParams): Promise<void> {
  if (openMethod === 'built-in-viewer' && window.ImageViewerAndroid?.openViewer) {
    const resolvedUris = allUris ?? (getAllUris ? await getAllUris() : await getMediaStoreUris());
    const viewerUris = resolvedUris.length > 0 ? resolvedUris : [filePath];
    window.ImageViewerAndroid.openViewer(filePath, JSON.stringify(viewerUris));
    void sendExifToViewer(filePath);
    return;
  }

  if (window.PermissionAndroid?.openImageWithChooser) {
    window.PermissionAndroid.openImageWithChooser(filePath);
    return;
  }

  await invoke('open_preview_window', { filePath });
}
