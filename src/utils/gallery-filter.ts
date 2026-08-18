/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import type { MediaItemDto } from '../types';
import { RAW_EXTENSIONS } from './raw';

/** Concrete file-type categories shown as filter buttons. */
export type GalleryFileCategory = 'jpeg' | 'heif' | 'raw' | 'other';

/** Filter mode held by the gallery, including the default "show all". */
export type GalleryFilterMode = 'all' | GalleryFileCategory;

const JPEG_EXTENSIONS = new Set(['jpg', 'jpeg']);
const HEIF_EXTENSIONS = new Set(['heif', 'hif', 'heic']);

/** Render order for the per-category filter buttons (after "全部"). */
export const FILTER_CATEGORY_ORDER: GalleryFileCategory[] = ['jpeg', 'heif', 'raw', 'other'];

/** Human-readable label for each category. */
export const FILTER_CATEGORY_LABEL: Record<GalleryFileCategory, string> = {
  jpeg: 'JPEG',
  heif: 'HEIF',
  raw: 'RAW',
  other: '其它',
};

/**
 * Extract the lowercase file extension from a media item. Falls back across
 * filePath → displayName → uri so classification works even when the preferred
 * field is null (e.g. test fixtures, partial provider rows). Returns '' when no
 * extension is present.
 */
function getFileExtension(item: MediaItemDto): string {
  for (const candidate of [item.filePath, item.displayName, item.uri]) {
    if (!candidate) continue;
    const dotIndex = candidate.lastIndexOf('.');
    // dotIndex <= 0 ⇒ no dot or a leading-dot name; consistent with isRawFile.
    if (dotIndex > 0 && dotIndex < candidate.length - 1) {
      return candidate.slice(dotIndex + 1).toLowerCase();
    }
  }
  return '';
}

/** Classify a media item into a single gallery file category. */
export function classifyFile(item: MediaItemDto): GalleryFileCategory {
  const ext = getFileExtension(item);
  if (JPEG_EXTENSIONS.has(ext)) return 'jpeg';
  if (HEIF_EXTENSIONS.has(ext)) return 'heif';
  if (RAW_EXTENSIONS.has(ext)) return 'raw';
  return 'other';
}
