/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useEffect, useRef } from 'react';
import type { AndroidImageOpenMethod } from '../types';
import type { MediaItemDto } from '../types';
import { openImagePreview } from '../services/image-open';

interface UseAndroidAutoOpenLatestPhotoParams {
  galleryItems: MediaItemDto[];
  openMethod?: AndroidImageOpenMethod;
  autoOpenLatestWhenVisible?: boolean;
}

interface GalleryItemsAddedEvent extends CustomEvent {
  detail: { items: MediaItemDto[]; timestamp: number };
}

export function useAndroidAutoOpenLatestPhoto({
  galleryItems,
  openMethod,
  autoOpenLatestWhenVisible,
}: UseAndroidAutoOpenLatestPhotoParams): void {
  const galleryItemsRef = useRef(galleryItems);
  const openMethodRef = useRef(openMethod);
  const autoOpenWhenVisibleRef = useRef(autoOpenLatestWhenVisible);

  galleryItemsRef.current = galleryItems;
  openMethodRef.current = openMethod;
  autoOpenWhenVisibleRef.current = autoOpenLatestWhenVisible;

  useEffect(() => {
    const handleItemsAdded = (event: GalleryItemsAddedEvent) => {
      if (openMethodRef.current !== 'built-in-viewer' || autoOpenWhenVisibleRef.current !== true) {
        return;
      }

      if (!window.ImageViewerAndroid?.isAppVisible?.()) {
        return;
      }

      const { items } = event.detail;
      if (!items || items.length === 0) {
        return;
      }

      const newest = items[items.length - 1];
      const allUris: string[] = [];
      const seenUris = new Set<string>();
      const addedUris = new Set<string>();

      for (const item of items) {
        if (seenUris.has(item.uri)) {
          continue;
        }
        seenUris.add(item.uri);
        addedUris.add(item.uri);
        allUris.push(item.uri);
      }

      for (const item of galleryItemsRef.current) {
        if (seenUris.has(item.uri)) {
          continue;
        }
        seenUris.add(item.uri);
        allUris.push(item.uri);
      }

      galleryItemsRef.current = [
        ...items,
        ...galleryItemsRef.current.filter((item) => !addedUris.has(item.uri)),
      ];

      void openImagePreview({
        filePath: newest.uri,
        openMethod: openMethodRef.current,
        allUris,
      });
    };

    window.addEventListener('gallery-items-added', handleItemsAdded as EventListener);
    return () => {
      window.removeEventListener('gallery-items-added', handleItemsAdded as EventListener);
    };
  }, []);
}
