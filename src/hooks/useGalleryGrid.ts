/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

// Legacy hook — thumbnail loading has moved to useThumbnailScheduler (V2 pipeline).
// This stub is kept for backward compatibility; prefer useThumbnailScheduler for new code.

import { useCallback, useState } from 'react';

type UseGalleryGridResult = {
  thumbnails: Map<string, string>;
  loadingThumbnails: Set<string>;
  imageRefCallback: (imagePath: string, el: HTMLDivElement | null) => void;
  removeThumbnailEntries: (imagePaths: Set<string>) => void;
  cleanupDeletedThumbnails: (imagePaths: Set<string>) => Promise<void>;
};

export function useGalleryGrid(): UseGalleryGridResult {
  const [thumbnails] = useState<Map<string, string>>(new Map());
  const [loadingThumbnails] = useState<Set<string>>(new Set());

  const imageRefCallback = useCallback((_imagePath: string, _el: HTMLDivElement | null) => {
    // No-op: V2 uses VirtualGalleryGrid + useThumbnailScheduler instead
  }, []);

  const removeThumbnailEntries = useCallback((_imagePaths: Set<string>) => {
    // No-op: V2 uses scheduler.removeThumbs() instead
  }, []);

  const cleanupDeletedThumbnails = useCallback(async (_imagePaths: Set<string>) => {
    // No-op: V2 uses scheduler.removeThumbs() instead
  }, []);

  return {
    thumbnails,
    loadingThumbnails,
    imageRefCallback,
    removeThumbnailEntries,
    cleanupDeletedThumbnails,
  };
}
