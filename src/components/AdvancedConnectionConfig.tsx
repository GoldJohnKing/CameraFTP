import { useState, useEffect } from 'react';
import { Loader2, Eye, EyeOff, ChevronDown, ChevronUp } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { ToggleSwitch } from './ui';
import type { AdvancedConnectionConfig } from '../types';

interface AdvancedConnectionConfigPanelProps {
  config: AdvancedConnectionConfig;
  port: number;
  autoSelectPort: boolean;
  isLoading: boolean;
  disabled?: boolean;
  onConfigChange: (config: AdvancedConnectionConfig) => Promise<void>;
  onPortChange: (port: number) => Promise<void>;
  onAutoSelectPortChange: (autoSelect: boolean) => Promise<void>;
}

export function AdvancedConnectionConfigPanel({
  config,
  port,
  autoSelectPort,
  isLoading,
  disabled = false,
  onConfigChange,
  onPortChange,
  onAutoSelectPortChange,
}: AdvancedConnectionConfigPanelProps) {
  const [isExpanded, setIsExpanded] = useState(config.enabled);
  const [portInput, setPortInput] = useState(port.toString());
  const [portError, setPortError] = useState<string | null>(null);
  const [isCheckingPort, setIsCheckingPort] = useState(false);
  const [showPassword, setShowPassword] = useState(false);
  const [pasvStartInput, setPasvStartInput] = useState(config.pasv.portStart.toString());
  const [pasvEndInput, setPasvEndInput] = useState(config.pasv.portEnd.toString());
  const [pasvError, setPasvError] = useState<string | null>(null);

  // Sync inputs when config changes
  useEffect(() => {
    setPortInput(port.toString());
  }, [port]);

  useEffect(() => {
    setPasvStartInput(config.pasv.portStart.toString());
    setPasvEndInput(config.pasv.portEnd.toString());
  }, [config.pasv.portStart, config.pasv.portEnd]);

  // Update expanded state when config.enabled changes externally
  useEffect(() => {
    setIsExpanded(config.enabled);
  }, [config.enabled]);

  const validatePort = (value: string): number | null => {
    const portNum = parseInt(value, 10);
    if (isNaN(portNum) || portNum < 1024 || portNum > 65535) {
      return null;
    }
    return portNum;
  };

  const validatePasvRange = (start: string, end: string): { start: number; end: number } | null => {
    const startNum = parseInt(start, 10);
    const endNum = parseInt(end, 10);
    if (isNaN(startNum) || isNaN(endNum)) return null;
    if (startNum < 1024 || endNum > 65535) return null;
    if (startNum >= endNum) return null;
    return { start: startNum, end: endNum };
  };

  const handleToggleEnabled = async () => {
    const newEnabled = !config.enabled;
    const newConfig = { ...config, enabled: newEnabled };
    await onConfigChange(newConfig);
    setIsExpanded(newEnabled);
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

  const handleAutoSelectToggle = async () => {
    await onAutoSelectPortChange(!autoSelectPort);
  };

  const handleAnonymousToggle = async () => {
    const newConfig = {
      ...config,
      auth: { ...config.auth, anonymous: !config.auth.anonymous },
    };
    await onConfigChange(newConfig);
  };

  const handleUsernameChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const newConfig = {
      ...config,
      auth: { ...config.auth, username: e.target.value },
    };
    onConfigChange(newConfig);
  };

  const handlePasswordChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const newConfig = {
      ...config,
      auth: { ...config.auth, password: e.target.value },
    };
    onConfigChange(newConfig);
  };

  const handlePasvToggle = async () => {
    const newConfig = {
      ...config,
      pasv: { ...config.pasv, enabled: !config.pasv.enabled },
    };
    await onConfigChange(newConfig);
  };

  const handlePasvStartChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const value = e.target.value;
    setPasvStartInput(value);
    const range = validatePasvRange(value, pasvEndInput);
    if (range === null) {
      setPasvError('起始端口必须小于结束端口，且都在 1024-65535 范围内');
    } else {
      setPasvError(null);
    }
  };

  const handlePasvEndChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const value = e.target.value;
    setPasvEndInput(value);
    const range = validatePasvRange(pasvStartInput, value);
    if (range === null) {
      setPasvError('结束端口必须大于起始端口，且都在 1024-65535 范围内');
    } else {
      setPasvError(null);
    }
  };

  const handlePasvBlur = async () => {
    const range = validatePasvRange(pasvStartInput, pasvEndInput);
    if (range === null) return;

    const newConfig = {
      ...config,
      pasv: {
        ...config.pasv,
        portStart: range.start,
        portEnd: range.end,
      },
    };
    await onConfigChange(newConfig);
  };

  return (
    <div className="space-y-4">
      {/* 高级连接配置开关 */}
      <ToggleSwitch
        enabled={config.enabled}
        onChange={handleToggleEnabled}
        label="高级连接配置"
        description="自定义认证方式和PASV模式（默认使用自动配置）"
        disabled={isLoading || disabled}
      />

      {/* 展开的配置面板 */}
      {config.enabled && (
        <div className="mt-4 space-y-6 border-t border-gray-100 pt-4">
          {/* 端口配置 */}
          <div className="space-y-3">
            <h4 className="text-sm font-semibold text-gray-800 flex items-center gap-2">
              端口配置
              <button
                onClick={() => setIsExpanded(!isExpanded)}
                className="text-gray-400 hover:text-gray-600"
              >
                {isExpanded ? <ChevronUp className="w-4 h-4" /> : <ChevronDown className="w-4 h-4" />}
              </button>
            </h4>

            <ToggleSwitch
              enabled={autoSelectPort}
              onChange={handleAutoSelectToggle}
              label="自动选择端口"
              description="自动寻找可用端口（推荐）"
              disabled={isLoading || disabled}
            />

            {!autoSelectPort && (
              <div className="space-y-2 pl-4 border-l-2 border-gray-100">
                <label className="block text-sm font-medium text-gray-700">
                  端口号
                </label>
                <div className="relative">
                  <input
                    type="number"
                    value={portInput}
                    onChange={handlePortChange}
                    onBlur={handlePortBlur}
                    placeholder="1024-65535"
                    disabled={isLoading || isCheckingPort || disabled}
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
                  <p className="text-xs text-red-600">{portError}</p>
                ) : (
                  <p className="text-xs text-gray-500">设置 FTP 服务器监听的端口号</p>
                )}
              </div>
            )}
          </div>

          {/* 认证配置 */}
          <div className="space-y-3">
            <h4 className="text-sm font-semibold text-gray-800">认证配置</h4>

            <ToggleSwitch
              enabled={config.auth.anonymous}
              onChange={handleAnonymousToggle}
              label="允许匿名访问"
              description="任何用户都可以无需密码连接"
              disabled={isLoading || disabled}
            />

            {!config.auth.anonymous && (
              <div className="space-y-3 pl-4 border-l-2 border-gray-100">
                <div className="space-y-2">
                  <label className="block text-sm font-medium text-gray-700">
                    用户名
                  </label>
                  <input
                    type="text"
                    value={config.auth.username}
                    onChange={handleUsernameChange}
                    placeholder="输入用户名"
                    disabled={isLoading || disabled}
                    className="w-full px-3 py-2 border border-gray-200 rounded-lg text-sm bg-white text-gray-700 disabled:opacity-50 disabled:cursor-not-allowed"
                  />
                </div>

                <div className="space-y-2">
                  <label className="block text-sm font-medium text-gray-700">
                    密码
                  </label>
                  <div className="relative">
                    <input
                      type={showPassword ? 'text' : 'password'}
                      value={config.auth.password}
                      onChange={handlePasswordChange}
                      placeholder="输入密码"
                      disabled={isLoading || disabled}
                      className="w-full px-3 py-2 border border-gray-200 rounded-lg text-sm bg-white text-gray-700 pr-10 disabled:opacity-50 disabled:cursor-not-allowed"
                    />
                    <button
                      type="button"
                      onClick={() => setShowPassword(!showPassword)}
                      disabled={isLoading || disabled}
                      className="absolute right-3 top-1/2 -translate-y-1/2 text-gray-400 hover:text-gray-600 disabled:opacity-50"
                    >
                      {showPassword ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
                    </button>
                  </div>
                </div>
              </div>
            )}
          </div>

          {/* PASV 配置 */}
          <div className="space-y-3">
            <h4 className="text-sm font-semibold text-gray-800">PASV 模式</h4>

            <ToggleSwitch
              enabled={config.pasv.enabled}
              onChange={handlePasvToggle}
              label="启用 PASV 模式"
              description="被动模式，适用于大多数网络环境"
              disabled={isLoading || disabled}
            />

            {config.pasv.enabled && (
              <div className="space-y-3 pl-4 border-l-2 border-gray-100">
                <div className="grid grid-cols-2 gap-3">
                  <div className="space-y-2">
                    <label className="block text-sm font-medium text-gray-700">
                      起始端口
                    </label>
                    <input
                      type="number"
                      value={pasvStartInput}
                      onChange={handlePasvStartChange}
                      onBlur={handlePasvBlur}
                      placeholder="50000"
                      disabled={isLoading || disabled}
                      className={`w-full px-3 py-2 border rounded-lg text-sm transition-colors ${
                        pasvError
                          ? 'border-red-300 bg-red-50 text-red-700'
                          : 'border-gray-200 bg-white text-gray-700'
                      } disabled:opacity-50 disabled:cursor-not-allowed`}
                    />
                  </div>
                  <div className="space-y-2">
                    <label className="block text-sm font-medium text-gray-700">
                      结束端口
                    </label>
                    <input
                      type="number"
                      value={pasvEndInput}
                      onChange={handlePasvEndChange}
                      onBlur={handlePasvBlur}
                      placeholder="50100"
                      disabled={isLoading || disabled}
                      className={`w-full px-3 py-2 border rounded-lg text-sm transition-colors ${
                        pasvError
                          ? 'border-red-300 bg-red-50 text-red-700'
                          : 'border-gray-200 bg-white text-gray-700'
                      } disabled:opacity-50 disabled:cursor-not-allowed`}
                    />
                  </div>
                </div>
                {pasvError ? (
                  <p className="text-xs text-red-600">{pasvError}</p>
                ) : (
                  <p className="text-xs text-gray-500">
                    设置 PASV 数据传输端口范围（默认 50000-50100）
                  </p>
                )}
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
