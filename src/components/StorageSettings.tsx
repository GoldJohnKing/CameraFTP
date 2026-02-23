import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Folder, AlertCircle, CheckCircle, Settings, RefreshCw } from 'lucide-react';
import { useStoragePermission } from '../hooks/useStoragePermission';

interface StorageSettingsProps {
  platform: string;
}

export function StorageSettings({ platform }: StorageSettingsProps) {
  const {
    storageInfo,
    isLoading,
    needsPermission,
    isReady,
    displayName,
    storagePath,
    loadStorageInfo,
    checkPermissionStatus,
    requestAllFilesPermission,
    ensureStorageReady,
  } = useStoragePermission();

  const [isCreatingDir, setIsCreatingDir] = useState(false);

  useEffect(() => {
    loadStorageInfo();
    checkPermissionStatus();
  }, [loadStorageInfo, checkPermissionStatus]);

  const handleRequestPermission = async () => {
    await requestAllFilesPermission();
  };

  const handleEnsureReady = async () => {
    setIsCreatingDir(true);
    try {
      await ensureStorageReady();
    } finally {
      setIsCreatingDir(false);
    }
  };

  const handleRefresh = async () => {
    await loadStorageInfo();
    await checkPermissionStatus();
  };

  // 桌面平台：显示简单的路径选择
  if (platform !== 'android') {
    return <DesktopStorageSettings />;
  }

  // Android 平台：固定路径 + 权限管理
  return (
    <div className="bg-white rounded-lg shadow-md p-4 mb-4">
      <h3 className="text-lg font-semibold mb-3 flex items-center gap-2">
        <Folder className="w-5 h-5" />
        存储设置
      </h3>

      <div className="space-y-4">
        {/* 固定路径显示 */}
        <div className="bg-gray-50 p-3 rounded-md">
          <label className="text-sm text-gray-600">存储位置（固定）</label>
          <div className="flex items-center gap-2 mt-1">
            <code className="text-sm bg-gray-100 px-2 py-1 rounded flex-1 break-all">
              {displayName}
            </code>
            {isReady ? (
              <CheckCircle className="w-5 h-5 text-green-500 flex-shrink-0" />
            ) : needsPermission ? (
              <AlertCircle className="w-5 h-5 text-yellow-500 flex-shrink-0" />
            ) : (
              <AlertCircle className="w-5 h-5 text-red-500 flex-shrink-0" />
            )}
          </div>
          <p className="text-xs text-gray-400 mt-1">{storagePath}</p>
        </div>

        {/* 权限状态 */}
        {needsPermission && (
          <div className="bg-yellow-50 border border-yellow-200 rounded-md p-3">
            <div className="flex items-start gap-2">
              <AlertCircle className="w-5 h-5 text-yellow-600 flex-shrink-0 mt-0.5" />
              <div className="flex-1">
                <p className="text-sm font-medium text-yellow-800">
                  需要存储权限
                </p>
                <p className="text-xs text-yellow-700 mt-1">
                  应用需要"所有文件访问权限"才能接收相机上传的照片
                </p>
                <button
                  onClick={handleRequestPermission}
                  className="mt-2 flex items-center gap-1 px-3 py-1.5 bg-yellow-600 text-white text-sm rounded-md hover:bg-yellow-700 transition-colors"
                >
                  <Settings className="w-4 h-4" />
                  前往设置授权
                </button>
              </div>
            </div>
          </div>
        )}

        {/* 目录不存在时显示创建按钮 */}
        {storageInfo && !storageInfo.exists && !needsPermission && (
          <div className="bg-blue-50 border border-blue-200 rounded-md p-3">
            <div className="flex items-start gap-2">
              <Folder className="w-5 h-5 text-blue-600 flex-shrink-0 mt-0.5" />
              <div className="flex-1">
                <p className="text-sm font-medium text-blue-800">
                  存储目录尚未创建
                </p>
                <p className="text-xs text-blue-700 mt-1">
                  点击下方按钮创建存储目录
                </p>
                <button
                  onClick={handleEnsureReady}
                  disabled={isCreatingDir}
                  className="mt-2 flex items-center gap-1 px-3 py-1.5 bg-blue-600 text-white text-sm rounded-md hover:bg-blue-700 disabled:opacity-50 transition-colors"
                >
                  {isCreatingDir ? (
                    <RefreshCw className="w-4 h-4 animate-spin" />
                  ) : (
                    <Folder className="w-4 h-4" />
                  )}
                  {isCreatingDir ? '创建中...' : '创建目录'}
                </button>
              </div>
            </div>
          </div>
        )}

        {/* 就绪状态 */}
        {isReady && (
          <div className="bg-green-50 border border-green-200 rounded-md p-3">
            <div className="flex items-center gap-2">
              <CheckCircle className="w-5 h-5 text-green-600" />
              <p className="text-sm text-green-800">
                存储已就绪，可以接收相机上传的照片
              </p>
            </div>
          </div>
        )}

        {/* 刷新按钮 */}
        <button
          onClick={handleRefresh}
          disabled={isLoading}
          className="w-full flex items-center justify-center gap-2 px-4 py-2 text-gray-600 bg-gray-100 rounded-md hover:bg-gray-200 disabled:opacity-50 transition-colors"
        >
          <RefreshCw className={`w-4 h-4 ${isLoading ? 'animate-spin' : ''}`} />
          刷新状态
        </button>

        {/* 说明 */}
        <div className="text-xs text-gray-500 bg-gray-50 p-2 rounded">
          <p>照片将保存到 {displayName}，可在相册中直接查看</p>
        </div>
      </div>
    </div>
  );
}

/// 桌面平台存储设置（支持自定义路径）
function DesktopStorageSettings() {
  const [currentPath, setCurrentPath] = useState<string>('');
  const [isSelecting, setIsSelecting] = useState(false);

  useEffect(() => {
    loadCurrentPath();
  }, []);

  const loadCurrentPath = async () => {
    try {
      const path = await invoke<string>('get_storage_path');
      setCurrentPath(path);
    } catch (e) {
      console.error('Failed to load storage path:', e);
    }
  };

  const handleSelectDirectory = async () => {
    setIsSelecting(true);
    try {
      const result = await invoke<string | null>('select_save_directory');
      if (result) {
        setCurrentPath(result);
        // 更新配置
        const config = await invoke<any>('load_config');
        config.save_path = result;
        await invoke('save_config', { config });
      }
    } catch (e) {
      console.error('Failed to select directory:', e);
    } finally {
      setIsSelecting(false);
    }
  };

  return (
    <div className="bg-white rounded-lg shadow-md p-4 mb-4">
      <h3 className="text-lg font-semibold mb-3 flex items-center gap-2">
        <Folder className="w-5 h-5" />
        存储路径设置
      </h3>

      <div className="space-y-4">
        <div className="bg-gray-50 p-3 rounded-md">
          <label className="text-sm text-gray-600">当前存储路径</label>
          <div className="flex items-center gap-2 mt-1">
            <code className="text-sm bg-gray-100 px-2 py-1 rounded flex-1 break-all">
              {currentPath || '未设置'}
            </code>
            <CheckCircle className="w-5 h-5 text-green-500 flex-shrink-0" />
          </div>
        </div>

        <button
          onClick={handleSelectDirectory}
          disabled={isSelecting}
          className="w-full flex items-center justify-center gap-2 bg-blue-600 text-white py-2 px-4 rounded-md hover:bg-blue-700 disabled:opacity-50 transition-colors"
        >
          {isSelecting ? (
            <RefreshCw className="w-4 h-4 animate-spin" />
          ) : (
            <Folder className="w-4 h-4" />
          )}
          {isSelecting ? '选择中...' : '更改目录'}
        </button>
      </div>
    </div>
  );
}
