import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Wifi, Copy, Check, Power } from 'lucide-react';
import { useServerStore } from '../stores/serverStore';

export function InfoCard() {
  const { serverInfo, isRunning } = useServerStore();
  const [copied, setCopied] = useState(false);
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

  const copyConnectionInfo = () => {
    if (!serverInfo) return;
    
    const info = `协议: FTP (被动模式)\n地址: ${serverInfo.url}\n用户名: ${serverInfo.username}\n密码: ${serverInfo.password_info}`;
    
    navigator.clipboard.writeText(info);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  if (!isRunning || !serverInfo) {
    return (
      <div className="bg-white rounded-xl shadow-sm border border-gray-200 p-6">
        <h2 className="text-lg font-semibold text-gray-800 mb-4">连接信息</h2>
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
          <span className="font-medium text-gray-800">FTP (被动模式)</span>
        </div>
        <div className="flex justify-between">
          <span className="text-gray-500">地址</span>
          <span className="font-medium text-gray-800 font-mono">
            {serverInfo.url}
          </span>
        </div>
        <div className="flex justify-between">
          <span className="text-gray-500">用户名</span>
          <span className="font-medium text-gray-800">
            {serverInfo.username}
          </span>
        </div>
        <div className="flex justify-between">
          <span className="text-gray-500">密码</span>
          <span className="font-medium text-gray-800">
            {serverInfo.password_info}
          </span>
        </div>
      </div>

      <button
        onClick={copyConnectionInfo}
        className="mt-4 w-full py-2 px-4 bg-gray-50 text-gray-700 rounded-lg flex items-center justify-center gap-2 hover:bg-gray-100 transition-colors"
      >
        {copied ? (
          <>
            <Check className="w-4 h-4" />
            已复制
          </>
        ) : (
          <>
            <Copy className="w-4 h-4" />
            复制连接信息
          </>
        )}
      </button>

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
