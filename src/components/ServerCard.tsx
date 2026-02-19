import { useState } from 'react';
import { Power, Loader2 } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { ServerInfo } from '../types';

interface ServerCardProps {
  onStatusChange: (info: ServerInfo | null) => void;
}

export function ServerCard({ onStatusChange }: ServerCardProps) {
  const [isRunning, setIsRunning] = useState(false);
  const [isLoading, setIsLoading] = useState(false);
  const [serverInfo, setServerInfo] = useState<ServerInfo | null>(null);
  const [error, setError] = useState<string | null>(null);

  const handleToggle = async () => {
    setIsLoading(true);
    setError(null);

    try {
      if (isRunning) {
        await invoke('stop_server');
        setIsRunning(false);
        setServerInfo(null);
        onStatusChange(null);
      } else {
        const info = await invoke<ServerInfo>('start_server');
        setIsRunning(true);
        setServerInfo(info);
        onStatusChange(info);
      }
    } catch (err) {
      setError(err as string);
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div className="bg-white rounded-xl shadow-sm border border-gray-200 p-6">
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-lg font-semibold text-gray-800">FTP服务器</h2>
        <div className={`w-3 h-3 rounded-full ${
          isRunning ? 'bg-green-500 animate-pulse' : 'bg-red-500'
        }`} />
      </div>

      <p className="text-gray-600 mb-6">
        {isRunning 
          ? `运行中 - ${serverInfo?.ip}:${serverInfo?.port}`
          : '服务器已停止，点击启动接收照片'
        }
      </p>

      <button
        onClick={handleToggle}
        disabled={isLoading}
        className={`w-full py-3 px-4 rounded-lg font-medium flex items-center justify-center gap-2 transition-colors ${
          isRunning
            ? 'bg-red-50 text-red-600 hover:bg-red-100'
            : 'bg-blue-600 text-white hover:bg-blue-700'
        } disabled:opacity-50 disabled:cursor-not-allowed`}
      >
        {isLoading ? (
          <Loader2 className="w-5 h-5 animate-spin" />
        ) : (
          <Power className="w-5 h-5" />
        )}
        {isRunning ? '停止服务器' : '启动服务器'}
      </button>

      {error && (
        <p className="mt-3 text-sm text-red-600 text-center">{error}</p>
      )}
    </div>
  );
}