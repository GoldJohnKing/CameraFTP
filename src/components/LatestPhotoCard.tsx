/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { memo, useCallback, useEffect, useState } from 'react';
import { Image } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { useServerStore } from '../stores/serverStore';
import { IconContainer } from './ui';
import type { FileInfo } from '../types';
import { LATEST_PHOTO_REFRESH_REQUESTED_EVENT } from '../utils/gallery-refresh';
import type { MediaStoreEntry } from '../utils/media-store-events';

interface FileIndexChangedEvent {
  count: number;
  latestFilename: string | null;
}

export const LatestPhotoCard = memo(function LatestPhotoCard() {
  const { stats } = useServerStore();
  const [scannedLatestFile, setScannedLatestFile] = useState<Pick<FileInfo, 'filename' | 'path'> | null>(null);

  const fetchLatestFile = useCallback(async () => {
    if (window.GalleryAndroid) {
      const listJson = await window.GalleryAndroid.listMediaStoreImages();
      const entries = JSON.parse(listJson ?? '[]') as MediaStoreEntry[];
      const latestEntry = entries[0] ?? null;

      return latestEntry
        ? {
            filename: latestEntry.displayName,
            path: latestEntry.uri,
          }
        : null;
    }

    return invoke<FileInfo | null>('get_latest_image');
  }, []);

  const refreshLatestFile = useCallback(async () => {
    try {
      const latest = await fetchLatestFile();
      setScannedLatestFile(latest);
    } catch (err) {
      console.error('[LatestPhotoCard] Failed to fetch latest image:', err);
    }
  }, [fetchLatestFile]);

  // 加载时获取扫描的最新文件
  useEffect(() => {
    void refreshLatestFile();
  }, [refreshLatestFile]);

  useEffect(() => {
    const handleRefreshRequest = (_event: Event) => {
      void refreshLatestFile();
    };

    window.addEventListener(LATEST_PHOTO_REFRESH_REQUESTED_EVENT, handleRefreshRequest);

    return () => {
      window.removeEventListener(LATEST_PHOTO_REFRESH_REQUESTED_EVENT, handleRefreshRequest);
    };
  }, [refreshLatestFile]);

  // 监听文件索引变化事件
  useEffect(() => {
    const unlistenPromise = listen<FileIndexChangedEvent>('file-index-changed', (event) => {
      if (event.payload.count === 0) {
        setScannedLatestFile(null);
      } else {
        void refreshLatestFile();
      }
    });

    return () => {
      unlistenPromise.then((unlisten) => unlisten()).catch(() => {});
    };
  }, [refreshLatestFile]);

  // 获取显示用的文件名
  // 优先使用实时扫描的文件（更及时地反映删除操作）
  const getFilename = () => {
    if (scannedLatestFile) {
      // 优先显示扫描到的文件（实时更新）
      return scannedLatestFile.filename;
    } else if (stats.lastFile) {
      // 回退到上传的文件
      const parts = stats.lastFile.split(/[\\/]/);
      return parts.pop() || stats.lastFile;
    }
    return '无';
  };

  const filename = getFilename();

  const handleOpenPreview = useCallback(async () => {
    try {
      const latest = await fetchLatestFile();
      if (latest) {
        setScannedLatestFile(latest);
        // 打开图片
        if (window.GalleryAndroid) {
          window.PermissionAndroid?.openImageWithChooser(latest.path);
        } else {
          await invoke('open_preview_window', { filePath: latest.path });
        }
      }
    } catch {
      // Silently ignore
    }
  }, [fetchLatestFile]);

  // 优先使用 scannedLatestFile 判断是否有文件（实时更新）
  const hasFile = scannedLatestFile || stats.lastFile;

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
