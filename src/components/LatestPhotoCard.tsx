import { memo, useCallback, useState, useEffect } from 'react';
import { Image } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { useServerStore } from '../stores/serverStore';
import { useConfigStore } from '../stores/configStore';
import { IconContainer } from './ui';
import type { FileInfo } from '../types';

export const LatestPhotoCard = memo(function LatestPhotoCard() {
  const { stats } = useServerStore();
  const { config } = useConfigStore();
  const [scannedLatestFile, setScannedLatestFile] = useState<FileInfo | null>(null);

  // 加载时获取扫描的最新文件
  useEffect(() => {
    const fetchLatestFile = async () => {
      try {
        const latest = await invoke<FileInfo | null>('get_latest_file');
        if (latest) {
          setScannedLatestFile(latest);
        }
      } catch (error) {
        console.error('Failed to get latest file:', error);
      }
    };
    fetchLatestFile();
  }, []);

  // 获取显示用的文件名
  const getFilename = () => {
    if (stats.lastFile) {
      // 优先显示上传的文件
      const parts = stats.lastFile.split(/[\\/]/);
      return parts.pop() || stats.lastFile;
    } else if (scannedLatestFile) {
      // 显示扫描到的文件
      return scannedLatestFile.filename;
    }
    return '无';
  };

  const filename = getFilename();

  const handleOpenPreview = useCallback(async () => {
    if (!config?.savePath) return;
    
    let targetPath: string | null = null;
    
    if (stats.lastFile) {
      // stats.lastFile 是相对路径，需要拼接 savePath
      targetPath = `${config.savePath}/${stats.lastFile}`.replace(/\\/g, '/');
    } else if (scannedLatestFile) {
      // scannedLatestFile.path 已经是完整路径
      targetPath = scannedLatestFile.path.replace(/\\/g, '/');
    }

    if (targetPath) {
      try {
        await invoke('open_preview_window', { filePath: targetPath });
      } catch (error) {
        console.error('Failed to open preview:', error);
      }
    }
  }, [stats.lastFile, scannedLatestFile, config?.savePath]);

  const hasFile = stats.lastFile || scannedLatestFile;

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
