/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import type { GalleryImage } from '../types';

/**
 * MediaStore entry returned by Android bridge
 */
export type MediaStoreEntry = {
  uri: string;
  displayName: string;
  dateModified: number;
  size?: number;
};

/**
 * Determines if the gallery should refresh based on the event type
 * @param event - The event name
 * @returns true if the gallery should refresh on this event
 */
export function shouldRefreshOnEvent(event: string): boolean {
  return event === 'media-store-ready';
}

/**
 * Converts a MediaStore entry to a GalleryImage
 * @param entry - The MediaStore entry from Android
 * @returns GalleryImage object for display
 */
export function toGalleryImage(entry: MediaStoreEntry): GalleryImage {
  return {
    path: entry.uri,
    filename: entry.displayName,
    sortTime: entry.dateModified,
  };
}
