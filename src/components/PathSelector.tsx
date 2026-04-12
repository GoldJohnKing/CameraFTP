/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useState, memo } from 'react';
import { Folder, RefreshCw } from 'lucide-react';
import type { StorageInfo } from '../types';
import { usePlatform } from '../hooks/usePlatform';

interface PathSelectorProps {
  storageInfo: StorageInfo | null;
  needsPermission: boolean;
  savePath: string | null;
  isLoading: boolean;
  disabled?: boolean;
  ensureStorageReady: () => Promise<{ success: boolean; error?: string }>;
  onSelectDirectory: () => Promise<void>;
}

export const PathSelector = memo(function PathSelector({
  storageInfo,
  needsPermission,
  savePath,
  isLoading,
  disabled = false,
  ensureStorageReady,
  onSelectDirectory,
}: PathSelectorProps) {
  const [isCreatingDir, setIsCreatingDir] = useState(false);

  const { isAndroid, isWindows: isDesktop } = usePlatform();

  const handleEnsureReady = async () => {
    setIsCreatingDir(true);
    try {
      await ensureStorageReady();
    } finally {
      setIsCreatingDir(false);
    }
  };

  // 获取显示的路径文本
  const getPathDisplay = () => {
    if (isAndroid) {
      return storageInfo?.displayName ?? 'DCIM/CameraFTP';
    }
    return savePath || '未设置';
  };

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between py-2">
        <div className="flex-1 min-w-0">
          <label className="block text-sm font-medium text-gray-700">
            存储路径
          </label>
          <p className="text-xs text-gray-500 mt-1 truncate">
            {getPathDisplay()}
          </p>
        </div>
        {/* 仅在桌面平台显示更改按钮 */}
        {isDesktop && (
          <button
            onClick={onSelectDirectory}
            disabled={isLoading || disabled}
            className="ml-3 shrink-0 px-3 py-1.5 text-sm bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            更改
          </button>
        )}
      </div>

      {/* Android 创建目录提示 */}
      {isAndroid && storageInfo && !storageInfo.exists && !needsPermission && (
        <div className="bg-blue-50 border border-blue-200 rounded-lg p-3">
          <div className="flex items-center gap-2">
            <Folder className="w-4 h-4 text-blue-600" />
            <p className="text-xs text-blue-800 flex-1">存储目录尚未创建</p>
            <button
              onClick={handleEnsureReady}
              disabled={isCreatingDir || disabled}
              className="flex items-center gap-1 px-2 py-1 bg-blue-600 text-white text-xs rounded hover:bg-blue-700 disabled:opacity-50 transition-colors"
            >
              {isCreatingDir ? (
                <RefreshCw className="w-3 h-3 animate-spin" />
              ) : (
                <Folder className="w-3 h-3" />
              )}
              {isCreatingDir ? '创建中...' : '创建'}
            </button>
          </div>
        </div>
      )}
    </div>
  );
});
