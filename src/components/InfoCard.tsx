import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Wifi, Power } from 'lucide-react';
import { useServerStore } from '../stores/serverStore';

export function InfoCard() {
  const { serverInfo, isRunning } = useServerStore();
  const [autostartEnabled, setAutostartEnabled] = useState(false);

  useEffect(() => {
    // 获取当前自启状态
    invoke('get_autostart_status')
      .then(status => setAutostartEnabled(status as boolean))
      .catch(console.error);
  }, []);

  const toggleAutostart = async () => {
    try {
      await invoke('set_autostart_command', { enable: !autostartEnabled });
      setAutostartEnabled(!autostartEnabled);
    } catch (e) {
      console.error('Failed to toggle autostart:', e);
    }
  };

  if (!isRunning || !serverInfo) {
    return (
      <div className="bg-white rounded-xl shadow-sm border border-gray-200 p-6">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-semibold text-gray-800">连接信息</h2>
          <div className="w-3 h-3 rounded-full bg-red-500" />
        </div>
        <p className="text-gray-500 text-center py-4">
          启动服务器后显示连接信息
        </p>
      </div>
    );
  }

  return (
    <div className="bg-white rounded-xl shadow-sm border border-gray-200 p-6">
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-lg font-semibold text-gray-800">连接信息</h2>
        <div className="w-10 h-10 rounded-lg bg-indigo-50 flex items-center justify-center">
          <Wifi className="w-5 h-5 text-indigo-600" />
        </div>
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

      {/* 自启动设置 */}
      <div className="mt-4 pt-4 border-t border-gray-200">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <Power className="w-4 h-4 text-gray-500" />
            <span className="text-sm text-gray-600">开机自启动</span>
          </div>
          <button
            onClick={toggleAutostart}
            className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
              autostartEnabled ? 'bg-blue-600' : 'bg-gray-200'
            }`}
          >
            <span
              className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                autostartEnabled ? 'translate-x-6' : 'translate-x-1'
              }`}
            />
          </button>
        </div>
      </div>
    </div>
  );
}
