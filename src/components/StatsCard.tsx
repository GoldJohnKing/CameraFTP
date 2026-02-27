import { memo, useCallback, useState, useEffect } from 'react';
import { Camera, Image, HardDrive, Clock } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { useServerStore } from '../stores/serverStore';
import { useConfigStore } from '../stores/configStore';
import { formatBytes } from '../utils/format';
import { Card, IconContainer } from './ui';
import type { FileInfo } from '../types';

export const StatsCard = memo(function StatsCard() {
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

  // 获取显示用的文件名和路径
  const getDisplayInfo = () => {
    if (stats.last_file) {
      // 优先显示上传的文件
      const parts = stats.last_file.split(/[\\/]/);
      return {
        filename: parts.pop() || stats.last_file,
        relativePath: parts.join('/')
      };
    } else if (scannedLatestFile) {
      // 显示扫描到的文件
      const relativeFullPath = scannedLatestFile.path.replace(config?.save_path || '', '').replace(/^[\\/]/, '');
      const parts = relativeFullPath.split(/[\\/]/);
      parts.pop(); // 移除文件名，只保留目录
      return {
        filename: scannedLatestFile.filename,
        relativePath: parts.join('/')
      };
    }
    return { filename: '无', relativePath: '' };
  };

  const { filename, relativePath } = getDisplayInfo();

  const handleOpenPreview = useCallback(async () => {
    if (!config?.save_path) return;
    
    let targetPath: string | null = null;
    
    if (stats.last_file) {
      // stats.last_file 是相对路径，需要拼接 save_path
      targetPath = `${config.save_path}/${stats.last_file}`.replace(/\\/g, '/');
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
  }, [stats.last_file, scannedLatestFile, config?.save_path]);

  return (
    <Card className="p-6">
      <h2 className="text-lg font-semibold text-gray-800 mb-4">传输统计</h2>

      <div className="space-y-4">
        <div className="flex items-center gap-3">
          <IconContainer color="blue">
            <Camera className="w-5 h-5 text-blue-600" />
          </IconContainer>
          <div>
            <p className="text-sm text-gray-500">相机状态</p>
            <p className="text-lg font-semibold text-gray-900">
              {stats.connected_clients > 0 ? '已连接' : '未连接'}
            </p>
          </div>
        </div>

        <div className="flex items-center gap-3">
          <IconContainer color="green">
            <Image className="w-5 h-5 text-green-600" />
          </IconContainer>
          <div>
            <p className="text-sm text-gray-500">已接收照片</p>
            <p className="text-lg font-semibold text-gray-800">
              {stats.files_received} 张
            </p>
          </div>
        </div>

        <div className="flex items-center gap-3">
          <IconContainer color="purple">
            <HardDrive className="w-5 h-5 text-purple-600" />
          </IconContainer>
          <div>
            <p className="text-sm text-gray-500">总数据量</p>
            <p className="text-lg font-semibold text-gray-800">
              {formatBytes(stats.bytes_received)}
            </p>
          </div>
        </div>

        {/* 最新文件区域 - 整体可点击按钮 */}
        <button
          onClick={handleOpenPreview}
          disabled={!stats.last_file && !scannedLatestFile}
          className={`
            w-full text-left p-3 rounded-lg border transition-colors
            ${(stats.last_file || scannedLatestFile)
              ? 'border-gray-200 hover:border-blue-300 hover:bg-blue-50 cursor-pointer'
              : 'border-gray-100 bg-gray-50 cursor-not-allowed'
            }
          `}
        >
          <div className="flex items-center gap-3">
            <IconContainer color="orange">
              <Clock className="w-5 h-5 text-orange-600" />
            </IconContainer>
            <div className="flex-1 min-w-0">
              <p className="text-xs text-gray-500 mb-0.5">最新照片</p>
              <p className={`text-base font-semibold truncate ${(stats.last_file || scannedLatestFile) ? 'text-gray-900' : 'text-gray-400'}`}>
                {filename}
              </p>
              {relativePath && (
                <p className="text-xs text-gray-400 truncate mt-0.5">
                  {relativePath}
                </p>
              )}
            </div>
          </div>
        </button>
      </div>
    </Card>
  );
});
