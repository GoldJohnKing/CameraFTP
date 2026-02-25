import { Wifi } from 'lucide-react';
import { useServerStore } from '../stores/serverStore';
import { Card, IconContainer } from './ui';

export function InfoCard() {
  const { serverInfo, isRunning } = useServerStore();

  if (!isRunning || !serverInfo) {
    return (
      <Card className="p-6">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-semibold text-gray-800">连接信息</h2>
          <div className="w-3 h-3 rounded-full bg-red-500" />
        </div>
        <p className="text-gray-500 text-center py-4">
          启动服务器后显示连接信息
        </p>
      </Card>
    );
  }

  return (
    <Card className="p-6">
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-lg font-semibold text-gray-800">连接信息</h2>
        <IconContainer color="indigo">
          <Wifi className="w-5 h-5 text-indigo-600" />
        </IconContainer>
      </div>

      <div className="space-y-3 text-sm">
        <div className="flex justify-between">
          <span className="text-gray-500">协议</span>
          <span className="font-medium text-gray-800">FTP (PASV 模式 / 被动模式)</span>
        </div>
        <div className="flex justify-between">
          <span className="text-gray-500">IP 地址</span>
          <span className="font-medium text-gray-800 font-mono">
            {serverInfo.ip}
          </span>
        </div>
        <div className="flex justify-between">
          <span className="text-gray-500">端口</span>
          <span className="font-medium text-gray-800 font-mono">
            {serverInfo.port}
          </span>
        </div>
        <div className="flex justify-between">
          <span className="text-gray-500">用户名 / 密码</span>
          <span className="font-medium text-gray-800">
            {serverInfo.username === 'anonymous' ? '匿名登陆 (任意用户名/密码)' : serverInfo.username}
          </span>
        </div>
      </div>
    </Card>
  );
}
