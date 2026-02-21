import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Folder, RefreshCw, AlertCircle, CheckCircle } from 'lucide-react';
import type { AppConfig } from '../types';

interface StorageSettingsProps {
  platform: string;
}

export function StorageSettings({ platform }: StorageSettingsProps) {
  const [currentPath, setCurrentPath] = useState<string>('');
  const [isSelecting, setIsSelecting] = useState(false);
  const [permissionStatus, setPermissionStatus] = useState<'valid' | 'invalid' | 'checking'>('checking');

  useEffect(() => {
    loadCurrentPath();

    // Android: 监听 SAF 选择结果
    if (platform === 'android') {
      const unlisten = listen<string>('android-directory-selected', (event) => {
        if (event.payload) {
          updateSavePath(event.payload);
        }
        setIsSelecting(false);
      });

      return () => {
        unlisten.then((fn) => fn());
      };
    }
  }, [platform]);

  const loadCurrentPath = async () => {
    try {
      const config = await invoke<AppConfig>('load_config');
      setCurrentPath(config.save_path);
      checkPermission(config.save_path);
    } catch (e) {
      console.error('Failed to load config:', e);
    }
  };

  const checkPermission = async (path: string) => {
    setPermissionStatus('checking');
    try {
      const isValid = await invoke<boolean>('validate_save_path', { path });
      setPermissionStatus(isValid ? 'valid' : 'invalid');
    } catch (e) {
      setPermissionStatus('invalid');
    }
  };

  const updateSavePath = async (newPath: string) => {
    try {
      const config = await invoke<AppConfig>('load_config');
      config.save_path = newPath;
      await invoke('save_config', { config });
      setCurrentPath(newPath);
      await checkPermission(newPath);
    } catch (e) {
      console.error('Failed to save config:', e);
      alert('保存配置失败: ' + e);
    }
  };

  const handleSelectDirectory = async () => {
    setIsSelecting(true);
    try {
      if (platform === 'android') {
        // Android: 发送事件给原生层打开 SAF 选择器
        // 这需要 MainActivity 配合处理
        if ((window as any).MainActivity?.openDirectoryPicker) {
          (window as any).MainActivity.openDirectoryPicker((uri: string | null) => {
            if (uri) {
              updateSavePath(uri);
            }
            setIsSelecting(false);
          });
        } else {
          // 备用方案：直接调用 Rust 命令
          const result = await invoke<string | null>('select_save_directory');
          if (result) {
            await updateSavePath(result);
          }
          setIsSelecting(false);
        }
      } else {
        // 桌面平台
        const result = await invoke<string | null>('select_save_directory');
        if (result) {
          await updateSavePath(result);
        }
        setIsSelecting(false);
      }
    } catch (e) {
      console.error('Failed to select directory:', e);
      alert('选择目录失败: ' + e);
      setIsSelecting(false);
    }
  };

  const handleUseRecommended = async () => {
    try {
      const recommended = await invoke<string>('get_recommended_save_path');
      if (recommended) {
        await updateSavePath(recommended);
      }
    } catch (e) {
      console.error('Failed to get recommended path:', e);
      alert('获取推荐路径失败: ' + e);
    }
  };

  return (
    <div className="bg-white rounded-lg shadow-md p-4 mb-4">
      <h3 className="text-lg font-semibold mb-3 flex items-center gap-2">
        <Folder className="w-5 h-5" />
        存储路径设置
      </h3>

      <div className="space-y-4">
        {/* 当前路径显示 */}
        <div className="bg-gray-50 p-3 rounded-md">
          <label className="text-sm text-gray-600">当前存储路径</label>
          <div className="flex items-center gap-2 mt-1">
            <code className="text-sm bg-gray-100 px-2 py-1 rounded flex-1 break-all">
              {currentPath || '未设置'}
            </code>
            {permissionStatus === 'valid' && (
              <CheckCircle className="w-5 h-5 text-green-500 flex-shrink-0" />
            )}
            {permissionStatus === 'invalid' && (
              <AlertCircle className="w-5 h-5 text-red-500 flex-shrink-0" />
            )}
          </div>
          {permissionStatus === 'invalid' && (
            <p className="text-xs text-red-500 mt-1">
              路径无效或权限已失效，请重新选择存储路径
            </p>
          )}
        </div>

        {/* 按钮组 */}
        <div className="flex gap-2">
          <button
            onClick={handleSelectDirectory}
            disabled={isSelecting}
            className="flex-1 bg-blue-600 text-white py-2 px-4 rounded-md hover:bg-blue-700
                       disabled:opacity-50 disabled:cursor-not-allowed flex items-center
                       justify-center gap-2"
          >
            {isSelecting ? (
              <RefreshCw className="w-4 h-4 animate-spin" />
            ) : (
              <Folder className="w-4 h-4" />
            )}
            {isSelecting ? '选择中...' : '更改目录'}
          </button>

          {platform === 'android' && (
            <button
              onClick={handleUseRecommended}
              className="flex-1 bg-gray-200 text-gray-700 py-2 px-4 rounded-md
                         hover:bg-gray-300 flex items-center justify-center gap-2"
            >
              <RefreshCw className="w-4 h-4" />
              使用推荐路径
            </button>
          )}
        </div>

        {/* Android 特有提示 */}
        {platform === 'android' && (
          <div className="text-xs text-gray-500 bg-blue-50 p-2 rounded">
            <p>提示：建议选择 DCIM/CameraFTPCompanion 或 Pictures/CameraFTPCompanion</p>
            <p>这样可以在相册中直接查看传输的照片</p>
          </div>
        )}
      </div>
    </div>
  );
}
