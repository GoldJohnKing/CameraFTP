import { useEffect, useState } from 'react';
import { Camera, Image, HardDrive, Clock } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { ServerStatus } from '../types';

export function StatsCard() {
  const [stats, setStats] = useState<ServerStatus>({
    is_running: false,
    connected_clients: 0,
    files_received: 0,
    bytes_received: 0,
    last_file: null,
  });

  useEffect(() => {
    // Initial load
    loadStats();

    // Set up polling
    const interval = setInterval(loadStats, 1000);

    // Listen for events
    const unlisten = listen('server-started', () => {
      loadStats();
    });

    return () => {
      clearInterval(interval);
      unlisten.then(f => f());
    };
  }, []);

  const loadStats = async () => {
    try {
      const status = await invoke<ServerStatus | null>('get_server_status');
      if (status) {
        setStats(status);
      }
    } catch (err) {
      console.error('Failed to load stats:', err);
    }
  };

  const formatBytes = (bytes: number): string => {
    if (bytes === 0) return '0 MB';
    const mb = bytes / (1024 * 1024);
    return `${mb.toFixed(1)} MB`;
  };

  return (
    <div className="bg-white rounded-xl shadow-sm border border-gray-200 p-6">
      <h2 className="text-lg font-semibold text-gray-800 mb-4">传输统计</h2>

      <div className="space-y-4">
        <div className="flex items-center gap-3">
          <div className="w-10 h-10 rounded-lg bg-blue-50 flex items-center justify-center">
            <Camera className="w-5 h-5 text-blue-600" />
          </div>
          <div>
            <p className="text-sm text-gray-500">已连接相机</p>
            <p className="text-lg font-semibold text-gray-800">
              {stats.connected_clients} 台
            </p>
          </div>
        </div>

        <div className="flex items-center gap-3">
          <div className="w-10 h-10 rounded-lg bg-green-50 flex items-center justify-center">
            <Image className="w-5 h-5 text-green-600" />
          </div>
          <div>
            <p className="text-sm text-gray-500">已接收照片</p>
            <p className="text-lg font-semibold text-gray-800">
              {stats.files_received} 张
            </p>
          </div>
        </div>

        <div className="flex items-center gap-3">
          <div className="w-10 h-10 rounded-lg bg-purple-50 flex items-center justify-center">
            <HardDrive className="w-5 h-5 text-purple-600" />
          </div>
          <div>
            <p className="text-sm text-gray-500">总数据量</p>
            <p className="text-lg font-semibold text-gray-800">
              {formatBytes(stats.bytes_received)}
            </p>
          </div>
        </div>

        {stats.last_file && (
          <div className="flex items-center gap-3 pt-4 border-t border-gray-100">
            <div className="w-10 h-10 rounded-lg bg-orange-50 flex items-center justify-center">
              <Clock className="w-5 h-5 text-orange-600" />
            </div>
            <div className="flex-1 min-w-0">
              <p className="text-sm text-gray-500">最新照片</p>
              <p className="text-sm font-medium text-gray-800 truncate">
                {stats.last_file}
              </p>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}