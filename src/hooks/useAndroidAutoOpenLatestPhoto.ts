/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useEffect, useRef } from 'react';
import type { AndroidImageOpenMethod } from '../types';
import type { MediaItemDto, GalleryItemsAddedEvent } from '../types';
import { openImagePreview } from '../services/image-open';

interface UseAndroidAutoOpenLatestPhotoParams {
  galleryItems: MediaItemDto[];
  openMethod?: AndroidImageOpenMethod;
  autoOpenLatestWhenVisible?: boolean;
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
      if (openMethodRef.current !== 'built-in-viewer') {
        return;
      }

      const { items } = event.detail;
      if (!items || items.length === 0) {
        return;
      }

      const bridge = window.ImageViewerAndroid;

      // Try incremental insertion into active viewer.
      // insertImage returns false when the viewer is not visible.
      const newest = items[items.length - 1];
      let viewerHandledInsertion = false;

      // Insert all new items at index 0 (newest first, matching MediaStore dateDesc order).
      // items[0]=oldest, items[n-1]=newest. Iterate oldest→newest, each at index 0,
      // so each newer item pushes previous ones right: newest ends up at 0.
      for (let i = 0; i < items.length; i++) {
        const inserted = bridge?.insertImage?.(items[i].uri, 0) ?? false;
        if (inserted) viewerHandledInsertion = true;
      }

      if (viewerHandledInsertion) {
        // Auto-navigate to the newest image only when config enabled
        if (autoOpenWhenVisibleRef.current === true) {
          bridge?.navigateToExistingUri?.(newest.uri);
        }
        return;
      }

      // Viewer is NOT active: only open if auto-view is enabled
      if (autoOpenWhenVisibleRef.current !== true) {
        return;
      }

      if (!bridge?.isAppVisible?.()) {
        return;
      }

      const allUris: string[] = [];
      const seenUris = new Set<string>();
      const addedUris = new Set<string>();

      for (const item of items) {
        if (seenUris.has(item.uri)) continue;
        seenUris.add(item.uri);
        addedUris.add(item.uri);
        allUris.push(item.uri);
      }

      for (const item of galleryItemsRef.current) {
        if (seenUris.has(item.uri)) continue;
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
