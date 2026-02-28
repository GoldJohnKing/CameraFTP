import { useState } from 'react';
import { Loader2, Eye, EyeOff, AlertCircle } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { ToggleSwitch } from './ui';
import type { AdvancedConnectionConfig } from '../types';

interface AdvancedConnectionConfigPanelProps {
  config: AdvancedConnectionConfig;
  port: number;
  platform: string;
  isLoading: boolean;
  disabled?: boolean;
  onUpdate: (updater: (draft: any) => Partial<any>) => void;
}

type PortValidationError = 
  | { type: 'empty' }
  | { type: 'invalid_number' }
  | { type: 'out_of_range'; min: number; max: number }
  | { type: 'port_in_use'; port: number };

type PasvValidationError =
  | { type: 'start_empty' }
  | { type: 'end_empty' }
  | { type: 'both_empty' }
  | { type: 'start_invalid' }
  | { type: 'end_invalid' }
  | { type: 'start_out_of_range' }
  | { type: 'end_out_of_range' }
  | { type: 'start_greater_than_end' };

export function AdvancedConnectionConfigPanel({
  config,
  port,
  platform,
  isLoading,
  disabled = false,
  onUpdate,
}: AdvancedConnectionConfigPanelProps) {
  // ========== 本地输入状态（完全独立，不从 props 同步）==========
  const [portInput, setPortInput] = useState(() => port.toString());
  const [portError, setPortError] = useState<PortValidationError | null>(null);
  const [isCheckingPort, setIsCheckingPort] = useState(false);
  const [showPassword, setShowPassword] = useState(false);
  const [pasvStartInput, setPasvStartInput] = useState(() => config.pasv.portStart.toString());
  const [pasvEndInput, setPasvEndInput] = useState(() => config.pasv.portEnd.toString());
  const [pasvError, setPasvError] = useState<PasvValidationError | null>(null);
  const [usernameInput, setUsernameInput] = useState(() => config.auth.username);
  const [passwordInput, setPasswordInput] = useState(() => config.auth.password);

  // Android 上禁止特权端口，Windows 上允许
  const minPort = platform === 'android' ? 1024 : 1;
  const maxPort = 65535;

  // ========== 错误消息 ==========
  const getPortErrorMessage = (error: PortValidationError): string => {
    switch (error.type) {
      case 'empty':
        return '端口号不能为空';
      case 'invalid_number':
        return '请输入有效的端口号';
      case 'out_of_range':
        return `端口号必须在 ${error.min}-${error.max} 之间`;
      case 'port_in_use':
        return `端口 ${error.port} 已被占用`;
    }
  };

  const getPasvErrorMessage = (error: PasvValidationError): string => {
    switch (error.type) {
      case 'start_empty':
        return '起始端口不能为空';
      case 'end_empty':
        return '结束端口不能为空';
      case 'both_empty':
        return '起始端口和结束端口不能为空';
      case 'start_invalid':
        return '起始端口不是有效的数字';
      case 'end_invalid':
        return '结束端口不是有效的数字';
      case 'start_out_of_range':
        return `起始端口必须在 ${minPort}-${maxPort} 之间`;
      case 'end_out_of_range':
        return `结束端口必须在 ${minPort}-${maxPort} 之间`;
      case 'start_greater_than_end':
        return '起始端口必须小于结束端口';
    }
  };

  // ========== 验证函数 ==========
  const validatePort = (value: string): { valid: boolean; port?: number; error?: PortValidationError } => {
    if (value.trim() === '') {
      return { valid: false, error: { type: 'empty' } };
    }
    
    const portNum = parseInt(value, 10);
    
    if (isNaN(portNum)) {
      return { valid: false, error: { type: 'invalid_number' } };
    }
    
    if (portNum < minPort || portNum > maxPort) {
      return { valid: false, error: { type: 'out_of_range', min: minPort, max: maxPort } };
    }
    
    return { valid: true, port: portNum };
  };

  const validatePasvRange = (start: string, end: string): { valid: boolean; startPort?: number; endPort?: number; error?: PasvValidationError } => {
    const startEmpty = start.trim() === '';
    const endEmpty = end.trim() === '';
    
    if (startEmpty && endEmpty) {
      return { valid: false, error: { type: 'both_empty' } };
    }
    if (startEmpty) {
      return { valid: false, error: { type: 'start_empty' } };
    }
    if (endEmpty) {
      return { valid: false, error: { type: 'end_empty' } };
    }
    
    const startNum = parseInt(start, 10);
    const endNum = parseInt(end, 10);
    
    if (isNaN(startNum)) {
      return { valid: false, error: { type: 'start_invalid' } };
    }
    if (isNaN(endNum)) {
      return { valid: false, error: { type: 'end_invalid' } };
    }
    
    if (startNum < minPort || startNum > maxPort) {
      return { valid: false, error: { type: 'start_out_of_range' } };
    }
    if (endNum < minPort || endNum > maxPort) {
      return { valid: false, error: { type: 'end_out_of_range' } };
    }
    
    if (startNum >= endNum) {
      return { valid: false, error: { type: 'start_greater_than_end' } };
    }
    
    return { valid: true, startPort: startNum, endPort: endNum };
  };

  // ========== 开关处理：立即更新 draft ==========
  const handleToggleEnabled = () => {
    onUpdate(() => ({
      advancedConnection: {
        ...config,
        enabled: !config.enabled,
      },
    }));
  };

  const handleAnonymousToggle = () => {
    onUpdate(() => ({
      advancedConnection: {
        ...config,
        auth: { ...config.auth, anonymous: !config.auth.anonymous },
      },
    }));
  };

  const handlePasvToggle = () => {
    onUpdate(() => ({
      advancedConnection: {
        ...config,
        pasv: { ...config.pasv, enabled: !config.pasv.enabled },
      },
    }));
  };

  // ========== 输入处理：仅更新本地 state ==========
  const handlePortChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const value = e.target.value;
    setPortInput(value);
    const result = validatePort(value);
    setPortError(result.valid ? null : result.error || null);
  };

  const handleUsernameChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    setUsernameInput(e.target.value);
  };

  const handlePasswordChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    setPasswordInput(e.target.value);
  };

  const handlePasvStartChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const value = e.target.value;
    setPasvStartInput(value);
    const result = validatePasvRange(value, pasvEndInput);
    setPasvError(result.valid ? null : result.error || null);
  };

  const handlePasvEndChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const value = e.target.value;
    setPasvEndInput(value);
    const result = validatePasvRange(pasvStartInput, value);
    setPasvError(result.valid ? null : result.error || null);
  };

  // ========== 失焦处理：更新 draft（触发防抖保存）==========
  const handlePortBlur = async () => {
    const result = validatePort(portInput);
    if (!result.valid || result.port === undefined) return;
    if (result.port === port) return;

    setIsCheckingPort(true);
    try {
      const isAvailable = await invoke<boolean>('check_port_available', { port: result.port });
      if (!isAvailable) {
        setPortError({ type: 'port_in_use', port: result.port });
        setIsCheckingPort(false);
        return;
      }
      // 更新 draft
      onUpdate(() => ({ port: result.port }));
    } catch {
      setPortError({ type: 'invalid_number' });
    } finally {
      setIsCheckingPort(false);
    }
  };

  const handleUsernameBlur = () => {
    if (usernameInput === config.auth.username) return;
    onUpdate(() => ({
      advancedConnection: {
        ...config,
        auth: { ...config.auth, username: usernameInput },
      },
    }));
  };

  const handlePasswordBlur = () => {
    if (passwordInput === config.auth.password) return;
    onUpdate(() => ({
      advancedConnection: {
        ...config,
        auth: { ...config.auth, password: passwordInput },
      },
    }));
  };

  const handlePasvBlur = () => {
    const result = validatePasvRange(pasvStartInput, pasvEndInput);
    if (!result.valid || result.startPort === undefined || result.endPort === undefined) return;
    if (result.startPort === config.pasv.portStart && result.endPort === config.pasv.portEnd) return;

    onUpdate(() => ({
      advancedConnection: {
        ...config,
        pasv: { ...config.pasv, portStart: result.startPort, portEnd: result.endPort },
      },
    }));
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
            <h4 className="text-sm font-semibold text-gray-800">端口配置</h4>

            <div className="space-y-2">
              <label className="block text-sm font-medium text-gray-700">
                端口号
              </label>
              <div className="relative">
                <input
                  type="number"
                  value={portInput}
                  onChange={handlePortChange}
                  onBlur={handlePortBlur}
                  placeholder={`${minPort}-${maxPort}`}
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
                <p className="text-xs text-red-600 flex items-center gap-1">
                  <AlertCircle className="w-3 h-3" />
                  {getPortErrorMessage(portError)}
                </p>
              ) : (
                <p className="text-xs text-gray-500">设置 FTP 服务器监听的端口号</p>
              )}
            </div>
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
                    value={usernameInput}
                    onChange={handleUsernameChange}
                    onBlur={handleUsernameBlur}
                    placeholder="输入用户名"
                    disabled={isLoading || disabled}
                    className={`w-full px-3 py-2 border rounded-lg text-sm transition-colors ${
                      usernameInput.trim() === '' && !config.auth.anonymous
                        ? 'border-red-300 bg-red-50'
                        : 'border-gray-200 bg-white'
                    } text-gray-700 disabled:opacity-50 disabled:cursor-not-allowed`}
                  />
                </div>

                <div className="space-y-2">
                  <label className="block text-sm font-medium text-gray-700">
                    密码
                  </label>
                  <div className="relative">
                    <input
                      type={showPassword ? 'text' : 'password'}
                      value={passwordInput}
                      onChange={handlePasswordChange}
                      onBlur={handlePasswordBlur}
                      placeholder="输入密码"
                      disabled={isLoading || disabled}
                      className={`w-full px-3 py-2 border rounded-lg text-sm transition-colors pr-10 ${
                        passwordInput === '' && !config.auth.anonymous
                          ? 'border-red-300 bg-red-50'
                          : 'border-gray-200 bg-white'
                      } text-gray-700 disabled:opacity-50 disabled:cursor-not-allowed`}
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

                {/* 凭据未完整配置警告 */}
                {(usernameInput.trim() === '' || passwordInput === '') && (
                  <p className="text-xs text-red-600 flex items-center gap-1">
                    <AlertCircle className="w-3 h-3" />
                    用户名或密码未配置，将回退到匿名访问模式
                  </p>
                )}
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
                        pasvError && (pasvError.type === 'start_empty' || pasvError.type === 'both_empty' || pasvError.type === 'start_invalid' || pasvError.type === 'start_out_of_range' || pasvError.type === 'start_greater_than_end')
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
                        pasvError && (pasvError.type === 'end_empty' || pasvError.type === 'both_empty' || pasvError.type === 'end_invalid' || pasvError.type === 'end_out_of_range' || pasvError.type === 'start_greater_than_end')
                          ? 'border-red-300 bg-red-50 text-red-700'
                          : 'border-gray-200 bg-white text-gray-700'
                      } disabled:opacity-50 disabled:cursor-not-allowed`}
                    />
                  </div>
                </div>
                {pasvError ? (
                  <p className="text-xs text-red-600 flex items-center gap-1">
                    <AlertCircle className="w-3 h-3" />
                    {getPasvErrorMessage(pasvError)}
                  </p>
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
