import { memo } from 'react';
import { Camera, Image, HardDrive, Clock } from 'lucide-react';
import { useServerStore } from '../stores/serverStore';
import { formatBytes } from '../utils/format';
import { Card, IconContainer } from './ui';

export const StatsCard = memo(function StatsCard() {
  const { stats } = useServerStore();

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

        {stats.last_file && (
          <div className="flex items-center gap-3 pt-4 border-t border-gray-100">
            <IconContainer color="orange">
              <Clock className="w-5 h-5 text-orange-600" />
            </IconContainer>
            <div className="flex-1 min-w-0">
              <p className="text-sm text-gray-500">最新照片</p>
              <p className="text-sm font-medium text-gray-800 truncate">
                {stats.last_file}
              </p>
            </div>
          </div>
        )}
      </div>
    </Card>
  );
});
