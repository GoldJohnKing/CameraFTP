import { useState, useEffect } from 'react';
import { Loader2 } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { ToggleSwitch } from './ui';

interface PortSelectorProps {
  autoSelectPort: boolean;
  port: number;
  isLoading: boolean;
  onAutoSelectToggle: (enabled: boolean) => Promise<void>;
  onPortChange: (port: number) => Promise<void>;
}

export function PortSelector({
  autoSelectPort,
  port,
  isLoading,
  onAutoSelectToggle,
  onPortChange,
}: PortSelectorProps) {
  const [portInput, setPortInput] = useState(port.toString());
  const [portError, setPortError] = useState<string | null>(null);
  const [isCheckingPort, setIsCheckingPort] = useState(false);

  // Sync port input with config
  useEffect(() => {
    setPortInput(port.toString());
  }, [port]);

  const validatePort = (value: string): number | null => {
    const portNum = parseInt(value, 10);
    if (isNaN(portNum) || portNum < 1024 || portNum > 65535) {
      return null;
    }
    return portNum;
  };

  const handleAutoSelectToggle = async () => {
    const newValue = !autoSelectPort;
    await onAutoSelectToggle(newValue);
  };

  const handlePortChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const value = e.target.value;
    setPortInput(value);

    const portNum = validatePort(value);
    if (portNum === null) {
      setPortError('端口号必须在 1024-65535 之间');
    } else {
      setPortError(null);
    }
  };

  const handlePortBlur = async () => {
    const portNum = validatePort(portInput);
    if (portNum === null) return;

    if (portNum === port) return;

    setIsCheckingPort(true);
    try {
      const isAvailable = await invoke<boolean>('check_port_available', { port: portNum });
      if (!isAvailable) {
        setPortError(`端口 ${portNum} 已被占用`);
        setIsCheckingPort(false);
        return;
      }

      await onPortChange(portNum);
    } catch {
      setPortError('检查端口时出错');
    } finally {
      setIsCheckingPort(false);
    }
  };

  return (
    <div className="space-y-3">
      <ToggleSwitch
        enabled={autoSelectPort}
        onChange={handleAutoSelectToggle}
        label="自动选择端口"
        description="自动寻找可用端口（推荐）"
        disabled={isLoading}
      />

      {/* 手动端口输入 */}
      {!autoSelectPort && (
        <div className="pl-4 border-l-2 border-gray-200">
          <label className="block text-sm font-medium text-gray-700 mb-2">
            端口号
          </label>
          <div className="relative">
            <input
              type="number"
              value={portInput}
              onChange={handlePortChange}
              onBlur={handlePortBlur}
              placeholder="1024-65535"
              disabled={isLoading || isCheckingPort}
              className={`w-full px-3 py-2 border rounded-lg text-sm transition-colors ${
                portError
                  ? 'border-red-300 bg-red-50 text-red-700'
                  : 'border-gray-200 bg-white text-gray-700'
              } disabled:opacity-50 disabled:cursor-not-allowed`}
            />
            {isCheckingPort && (
              <Loader2 className="w-4 h-4 animate-spin text-gray-400 absolute right-3 top-1/2 -translate-y-1/2" />
            )}
          </div>
          {portError ? (
            <p className="text-xs text-red-600 mt-1">{portError}</p>
          ) : (
            <p className="text-xs text-gray-500 mt-1">
              设置 FTP 服务器监听的端口号（1024-65535）
            </p>
          )}
        </div>
      )}
    </div>
  );
}
