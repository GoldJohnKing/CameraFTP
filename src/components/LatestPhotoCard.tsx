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

export const LatestPhotoCard = memo(function LatestPhotoCard() {
  const { stats } = useServerStore();
  const openPreview = useImagePreviewOpener();
  const { latestPhoto, refreshLatestPhoto } = useLatestPhoto();

  // 获取显示用的文件名
  // 优先使用实时扫描的文件（更及时地反映删除操作）
  const getFilename = () => {
    if (latestPhoto) {
      // 优先显示扫描到的文件（实时更新）
      return latestPhoto.filename;
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
        const latest = await refreshLatestPhoto();
        if (latest) {
          await openPreview({
            filePath: latest.path,
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
