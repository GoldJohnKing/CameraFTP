import { useState } from 'react';
import { Folder, RefreshCw } from 'lucide-react';
import type { StorageInfo } from '../types';

interface PathSelectorProps {
  platform: string;
  storageInfo: StorageInfo | null;
  needsPermission: boolean;
  savePath: string | null;
  isLoading: boolean;
  ensureStorageReady: () => Promise<{ success: boolean; error?: string }>;
  onSelectDirectory: () => Promise<void>;
}

export function PathSelector({
  platform,
  storageInfo,
  needsPermission,
  savePath,
  isLoading,
  ensureStorageReady,
  onSelectDirectory,
}: PathSelectorProps) {
  const [isCreatingDir, setIsCreatingDir] = useState(false);

  const isAndroid = platform === 'android';
  const isDesktop = platform === 'windows' || platform === 'macos' || platform === 'linux';

  const handleEnsureReady = async () => {
    setIsCreatingDir(true);
    try {
      await ensureStorageReady();
    } finally {
      setIsCreatingDir(false);
    }
  };

  return (
    <>
      {/* Android 存储设置 */}
      {isAndroid && (
        <div className="space-y-3">
          <div className="flex items-center justify-between py-2">
            <div className="flex-1 min-w-0">
              <label className="block text-sm font-medium text-gray-700">
                存储位置
              </label>
              <p className="text-xs text-gray-500 mt-1 truncate">{storageInfo?.display_name ?? 'DCIM/CameraFTP'}</p>
            </div>
          </div>

          {/* 创建目录 */}
          {storageInfo && !storageInfo.exists && !needsPermission && (
            <div className="bg-blue-50 border border-blue-200 rounded-lg p-3">
              <div className="flex items-center gap-2">
                <Folder className="w-4 h-4 text-blue-600" />
                <p className="text-xs text-blue-800 flex-1">存储目录尚未创建</p>
                <button
                  onClick={handleEnsureReady}
                  disabled={isCreatingDir}
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
      )}

      {/* 桌面端存储路径配置 */}
      {isDesktop && (
        <div className="space-y-3">
          <div className="flex items-center justify-between py-2">
            <div className="flex-1 min-w-0">
              <label className="block text-sm font-medium text-gray-700">
                存储路径
              </label>
              <p className="text-xs text-gray-500 mt-1 truncate">
                {savePath || '未设置'}
              </p>
            </div>
            <button
              onClick={onSelectDirectory}
              disabled={isLoading}
              className="ml-3 shrink-0 px-3 py-1.5 text-sm bg-gray-100 text-gray-700 rounded-lg hover:bg-gray-200 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
            >
              更改
            </button>
          </div>
        </div>
      )}
    </>
  );
}
