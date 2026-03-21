/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useState, useMemo } from 'react';
import { Loader2, Eye, EyeOff, AlertCircle } from 'lucide-react';
import { ToggleSwitch } from './ui';
import type { AdvancedConnectionConfig, AppConfig } from '../types';
import { validatePort as validatePortBasic } from '../utils/validation';
import { usePortCheck } from '../hooks/usePortCheck';
import { useConfigStore } from '../stores/configStore';

const PASSWORD_PLACEHOLDER = '••••••••';

// Note: ToggleSwitch import kept for "允许匿名访问" toggle

interface AdvancedConnectionConfigPanelProps {
  config: AdvancedConnectionConfig;
  port: number;
  platform: string;
  isLoading: boolean;
  disabled?: boolean;
  onUpdate: (updater: (draft: AppConfig) => Partial<AppConfig>) => void;
}

type PortValidationError = 
  | { type: 'empty' }
  | { type: 'invalid_number' }
  | { type: 'out_of_range'; min: number; max: number }
  | { type: 'port_in_use'; port: number };

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
  const [showPassword, setShowPassword] = useState(false);
  
  // ========== Port checking hook ==========
  const { checkPort, isChecking: isCheckingPort } = usePortCheck();
  const saveAuthConfig = useConfigStore(state => state.saveAuthConfig);
  const [usernameInput, setUsernameInput] = useState(() => config.auth.username);
  
  // ========== 密码状态（派生 + 编辑隔离）==========
  // 派生状态：是否已有保存的密码（始终与 config 同步，不检查 anonymous）
  const hasExistingPassword = useMemo(
    () => !!config.auth.passwordHash,
    [config.auth.passwordHash]
  );

  // 首次配置密码的乐观标记：保存成功后立即标记，避免等待 config 同步导致的闪烁
  // 只在首次配置时生效（hasExistingPassword 为 false 时）
  // 后续 config 同步后 hasExistingPassword 变为 true，此标记不再需要
  const [hasCompletedFirstSetup, setHasCompletedFirstSetup] = useState(false);
  const hasPassword = hasExistingPassword || hasCompletedFirstSetup;

  // 编辑状态：仅在用户正在编辑时使用
  const [isEditingPassword, setIsEditingPassword] = useState(false);
  const [editPasswordValue, setEditPasswordValue] = useState('');

  // 显示值：编辑中显示输入值，否则显示占位符或空
  const passwordDisplayValue = isEditingPassword
    ? editPasswordValue
    : (hasPassword ? PASSWORD_PLACEHOLDER : '');

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

  // ========== 验证函数 ==========
  const validatePort = (value: string): { valid: boolean; port?: number; error?: PortValidationError } => {
    if (value.trim() === '') {
      return { valid: false, error: { type: 'empty' } };
    }
    
    const portNum = validatePortBasic(value);
    
    if (portNum === null) {
      return { valid: false, error: { type: 'invalid_number' } };
    }
    
    if (portNum < minPort || portNum > maxPort) {
      return { valid: false, error: { type: 'out_of_range', min: minPort, max: maxPort } };
    }
    
    return { valid: true, port: portNum };
  };

  // ========== 开关处理：立即更新 draft ==========
  const handleAnonymousToggle = () => {
    onUpdate(() => ({
      advancedConnection: {
        ...config,
        auth: { ...config.auth, anonymous: !config.auth.anonymous },
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

  const handlePasswordFocus = () => {
    // 如果不在编辑模式，进入编辑模式
    if (!isEditingPassword) {
      setIsEditingPassword(true);
      // 清空编辑值，准备接收新输入
      setEditPasswordValue('');
    }
  };

  const handlePasswordChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    setEditPasswordValue(e.target.value);
  };

  // ========== 失焦处理：更新 draft（触发防抖保存）==========
  const handlePortBlur = async () => {
    // 如果为空，恢复原端口
    if (portInput.trim() === '') {
      setPortInput(port.toString());
      setPortError(null);
      return;
    }
    
    const result = validatePort(portInput);
    if (!result.valid || result.port === undefined) {
      // 验证失败，恢复原端口
      setPortInput(port.toString());
      setPortError(null);
      return;
    }
    if (result.port === port) return;

    const checkResult = await checkPort(portInput);
    if (!checkResult.available) {
      setPortError({ type: 'port_in_use', port: result.port });
      return;
    }
    // 更新 draft
    onUpdate(() => ({ port: result.port }));
  };

  const handleUsernameBlur = () => {
    // 如果为空，恢复原用户名
    if (usernameInput.trim() === '') {
      setUsernameInput(config.auth.username);
      return;
    }
    if (usernameInput === config.auth.username) return;
    onUpdate(() => ({
      advancedConnection: {
        ...config,
        auth: { ...config.auth, username: usernameInput },
      },
    }));
  };

  const handlePasswordBlur = async () => {
    // 如果不在编辑模式，不处理
    if (!isEditingPassword) return;

    // 如果输入为空，退出编辑模式并恢复原状
    if (editPasswordValue === '') {
      setIsEditingPassword(false);
      return;
    }

    try {
      // 传输明文密码，后端进行 Argon2id 哈希
      await saveAuthConfig({
        anonymous: config.auth.anonymous,
        username: usernameInput,
        password: editPasswordValue,
      });

      // 首次配置密码成功后立即标记，避免红色边框闪烁
      if (!hasExistingPassword) {
        setHasCompletedFirstSetup(true);
      }
      setIsEditingPassword(false);
      setEditPasswordValue('');
      setShowPassword(false);
    } catch (error) {
      console.error('Failed to save auth config:', error);
      setIsEditingPassword(false);
    }
  };

  return (
    <div className="space-y-6">
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
              className={`w-full px-3 py-2 border rounded-lg text-sm ${
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
            <div className="grid grid-cols-2 gap-3">
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
                  className={`w-full px-3 py-2 border rounded-lg text-sm ${
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
                    value={passwordDisplayValue}
                    onChange={handlePasswordChange}
                    onFocus={handlePasswordFocus}
                    onBlur={handlePasswordBlur}
                    placeholder="输入密码"
                    disabled={isLoading || disabled}
                    className={`w-full px-3 py-2 border rounded-lg text-sm pr-10 ${
                      !hasPassword && !isEditingPassword
                        ? 'border-red-300 bg-red-50'
                        : 'border-gray-200 bg-white'
                    } text-gray-700 disabled:opacity-50 disabled:cursor-not-allowed`}
                  />
                  {/* 编辑模式或无已保存密码时显示预览按钮 */}
                  {(isEditingPassword || !hasPassword) && (
                    <button
                      type="button"
                      onMouseDown={(e) => e.preventDefault()} // 阻止点击时输入框失焦
                      onClick={() => setShowPassword(!showPassword)}
                      disabled={isLoading || disabled}
                      className="absolute right-3 top-1/2 -translate-y-1/2 text-gray-400 hover:text-gray-600 disabled:opacity-50"
                    >
                      {showPassword ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
                    </button>
                  )}
                </div>
              </div>
            </div>

            {/* 凭据未完整配置警告 */}
            {(usernameInput.trim() === '' || (!hasPassword && editPasswordValue === '')) && (
              <p className="text-xs text-red-600 flex items-center gap-1">
                <AlertCircle className="w-3 h-3" />
                用户名或密码未配置，将使用匿名访问模式
              </p>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
