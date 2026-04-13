/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { memo, useCallback } from 'react';
import { Image } from 'lucide-react';
import { useServerStore } from '../stores/serverStore';
import { IconContainer } from './ui';
import { useImagePreviewOpener } from '../hooks/useImagePreviewOpener';
import { useLatestPhoto } from '../hooks/useLatestPhoto';
import { isGalleryV2Available, listMediaPage, GALLERY_PAGE_SIZE } from '../services/gallery-media-v2';

async function getAllUrisFromGalleryV2(): Promise<string[]> {
  if (!isGalleryV2Available()) {
    return [];
  }

  const uris: string[] = [];
  const seen = new Set<string>();
  let cursor: string | null = null;

  while (true) {
    const page = await listMediaPage({
      cursor,
      pageSize: GALLERY_PAGE_SIZE,
      sort: 'dateDesc',
    });

    for (const item of page.items) {
      if (seen.has(item.uri)) {
        continue;
      }
      seen.add(item.uri);
      uris.push(item.uri);
    }

    if (page.nextCursor === null) {
      break;
    }

    cursor = page.nextCursor;
  }

  return uris;
}

export const LatestPhotoCard = memo(function LatestPhotoCard() {
  const { stats } = useServerStore();
  const openPreview = useImagePreviewOpener();
  const { latestPhoto, refreshLatestPhoto } = useLatestPhoto();

  const filename = latestPhoto?.filename
    ?? (stats.lastFile ? stats.lastFile.split(/[\\/]/).pop() || stats.lastFile : '无');

  const handleOpenPreview = useCallback(async () => {
    try {
        const latest = await refreshLatestPhoto();
        if (latest) {
          await openPreview({
            filePath: latest.path,
            getAllUris: getAllUrisFromGalleryV2,
          });
        }
      } catch {
        // Silently ignore
      }
  }, [openPreview, refreshLatestPhoto]);

  // 优先使用 latestPhoto 判断是否有文件（实时更新）
  const hasFile = latestPhoto || stats.lastFile;

  return (
    <button
      onClick={handleOpenPreview}
      disabled={!hasFile}
      className={`
        w-full text-left p-4 rounded-xl border bg-white shadow-sm transition-colors
        ${hasFile
          ? 'border-gray-200 hover:border-blue-300 hover:bg-blue-50 cursor-pointer'
          : 'border-gray-100 bg-gray-50 cursor-not-allowed opacity-60'
        }
      `}
    >
      <div className="flex items-center gap-3">
        <IconContainer color="orange">
          <Image className="w-5 h-5 text-orange-600" />
        </IconContainer>
        <div className="flex-1 min-w-0">
          <p className="text-sm text-gray-500">最新照片</p>
          <p className={`text-base font-semibold truncate ${hasFile ? 'text-gray-900' : 'text-gray-400'}`}>
            {filename}
          </p>

        </div>
      </div>
    </button>
  );
});
