import { useEffect, useState, useCallback } from 'react';
import { Loader2, Folder, RefreshCw } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { useConfigStore } from '../stores/configStore';
import { usePermissionStore } from '../stores/permissionStore';
import { useStoragePermission } from '../hooks/useStoragePermission';
import { Card, CardHeader, ToggleSwitch } from './ui';

export function ConfigCard() {
  const {
    config,
    isLoading,
    error,
    platform,
    loadConfig,
    loadPlatform,
    setAutostart,
    updatePort,
    updateAutoSelectPort,
  } = useConfigStore();

  const {
    storageInfo,
    isLoading: isLoadingStorage,
    needsPermission,
    displayName,
    ensureStorageReady,
  } = useStoragePermission();

  const permissionState = usePermissionStore((state) => state.permissions);
  const checkPermissions = usePermissionStore((state) => state.checkPermissions);
  const requestStoragePermission = usePermissionStore((state) => state.requestStoragePermission);
  const requestNotificationPermission = usePermissionStore((state) => state.requestNotificationPermission);
  const requestBatteryOptimization = usePermissionStore((state) => state.requestBatteryOptimization);

  const [autostartEnabled, setAutostartEnabled] = useState(false);
  const [isLoadingAutostart, setIsLoadingAutostart] = useState(false);
  const [portInput, setPortInput] = useState('2121');
  const [portError, setPortError] = useState<string | null>(null);
  const [isCheckingPort, setIsCheckingPort] = useState(false);
  const [isCreatingDir, setIsCreatingDir] = useState(false);
  const [isChecking, setIsChecking] = useState(false);

  // Platform detection
  const isDesktop = platform === 'windows' || platform === 'macos' || platform === 'linux';
  const isAndroid = platform === 'android';

  useEffect(() => {
    const isCancelled = { current: false };
    
    loadConfig();
    loadPlatform();
    loadAutostartStatus(isCancelled);
    
    return () => {
      isCancelled.current = true;
    };
  }, [loadConfig, loadPlatform]);

  // 仅在组件挂载时检查一次权限（用户进入Config界面）
  // 之后不再自动刷新，依赖用户手动刷新
  useEffect(() => {
    if (isAndroid) {
      checkPermissions();
    }
    // 注意：依赖项为空数组，仅在挂载时执行
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Sync port input with config
  useEffect(() => {
    if (config) {
      setPortInput(config.port.toString());
    }
  }, [config?.port]);

  const loadAutostartStatus = async (isCancelled: { current: boolean }) => {
    try {
      const status = await invoke<boolean>('get_autostart_status');
      if (!isCancelled.current) {
        setAutostartEnabled(status);
      }
    } catch {
      // Silently ignore autostart status load errors
    }
  };

  // Wrapper for checkPermissions with loading state
  const handleCheckPermissions = useCallback(async () => {
    setIsChecking(true);
    try {
      await checkPermissions();
    } finally {
      setTimeout(() => setIsChecking(false), 300);
    }
  }, [checkPermissions]);

  const handleAutostartToggle = async () => {
    setIsLoadingAutostart(true);
    try {
      const newValue = !autostartEnabled;
      await setAutostart(newValue);
      setAutostartEnabled(newValue);
    } finally {
      setIsLoadingAutostart(false);
    }
  };

  const handleAutoSelectToggle = async () => {
    if (!config) return;
    const newValue = !config.auto_select_port;
    await updateAutoSelectPort(newValue);
  };

  const validatePort = (value: string): number | null => {
    const port = parseInt(value, 10);
    if (isNaN(port) || port < 1024 || port > 65535) {
      return null;
    }
    return port;
  };

  const handlePortChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const value = e.target.value;
    setPortInput(value);

    const port = validatePort(value);
    if (port === null) {
      setPortError('端口号必须在 1024-65535 之间');
    } else {
      setPortError(null);
    }
  };

  const handlePortBlur = async () => {
    const port = validatePort(portInput);
    if (port === null || !config) return;

    if (port === config.port) return;

    setIsCheckingPort(true);
    try {
      const isAvailable = await invoke<boolean>('check_port_available', { port });
      if (!isAvailable) {
        setPortError(`端口 ${port} 已被占用`);
        setIsCheckingPort(false);
        return;
      }

      await updatePort(port);
    } catch {
      setPortError('检查端口时出错');
    } finally {
      setIsCheckingPort(false);
    }
  };

  const handleEnsureReady = async () => {
    setIsCreatingDir(true);
    try {
      await ensureStorageReady();
    } finally {
      setIsCreatingDir(false);
    }
  };

  return (
  <>
    <Card className="overflow-hidden">
      <CardHeader title="应用配置" description="管理应用设置和偏好" />

      <div className="p-4 space-y-6">
        {/* Android 存储设置 */}
        {isAndroid && (
          <div className="space-y-3">
            <div className="flex items-center justify-between py-2">
              <div className="flex-1 min-w-0">
                <label className="block text-sm font-medium text-gray-700">
                  存储位置
                </label>
                {isLoadingStorage ? (
                  <p className="text-xs text-gray-400 mt-1">加载中...</p>
                ) : (
                  <p className="text-xs text-gray-500 mt-1 truncate">{displayName}</p>
                )}
              </div>
            </div>

            {/* 创建目录 */}
            {storageInfo && !storageInfo.exists && !needsPermission && (
              <div className="bg-blue-50 border border-blue-200 rounded-lg p-3">
                <div className="flex items-center gap-2">
                  <Folder className="w-4 h-4 text-blue-600" />
                  <p className="text-xs text-blue-800 flex-1">存储目录尚未创建</p>
                  <button
                    onClick={handleEnsureReady}
                    disabled={isCreatingDir}
                    className="flex items-center gap-1 px-2 py-1 bg-blue-600 text-white text-xs rounded hover:bg-blue-700 disabled:opacity-50 transition-colors"
                  >
                    {isCreatingDir ? (
                      <RefreshCw className="w-3 h-3 animate-spin" />
                    ) : (
                      <Folder className="w-3 h-3" />
                    )}
                    {isCreatingDir ? '创建中...' : '创建'}
                  </button>
                </div>
              </div>
            )}
          </div>
        )}

        {/* 桌面端存储路径配置 */}
        {isDesktop && (
          <div className="space-y-3">
            <div className="flex items-center justify-between py-2">
              <div className="flex-1 min-w-0">
                <label className="block text-sm font-medium text-gray-700">
                  存储路径
                </label>
                <p className="text-xs text-gray-500 mt-1 truncate">
                  {config?.save_path?.toString() || '未设置'}
                </p>
              </div>
              <button
                onClick={async () => {
                  const result = await invoke<string | null>('select_save_directory');
                  if (result && config) {
                    const newConfig = { ...config, save_path: result };
                    await invoke('save_config', { config: newConfig });
                    loadConfig();
                  }
                }}
                disabled={isLoading}
                className="ml-3 shrink-0 px-3 py-1.5 text-sm bg-gray-100 text-gray-700 rounded-lg hover:bg-gray-200 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
              >
                更改
              </button>
            </div>
          </div>
        )}

        {/* 开机自启动配置 - 仅在桌面平台显示 */}
        {isDesktop && (
          <ToggleSwitch
            enabled={autostartEnabled}
            onChange={handleAutostartToggle}
            label="开机自启动"
            description="系统启动时自动运行图传伴侣"
            disabled={isLoadingAutostart}
          />
        )}

        {/* 端口配置 */}
        <div className="space-y-3">
          <ToggleSwitch
            enabled={config?.auto_select_port ?? true}
            onChange={handleAutoSelectToggle}
            label="自动选择端口"
            description="自动寻找可用端口（推荐）"
            disabled={isLoading}
          />

          {/* 手动端口输入 */}
          {!config?.auto_select_port && (
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

        {/* 错误提示 */}
        {error && (
          <div className="p-3 bg-red-50 border border-red-200 rounded-lg">
            <p className="text-sm text-red-600">{error}</p>
          </div>
        )}
      </div>
    </Card>

    {/* Permission Status Section - Android Only */}
    {isAndroid && typeof window !== 'undefined' && window.PermissionAndroid && (
      <Card className="overflow-hidden">
        <div className="p-4 border-b border-gray-100">
          <div className="flex items-center justify-between">
            <h3 className="text-sm font-medium text-gray-700">权限状态</h3>
            <button
              onClick={handleCheckPermissions}
              disabled={isChecking}
              className="text-xs text-blue-500 hover:text-blue-600 flex items-center gap-1 disabled:opacity-50"
            >
              <svg 
                className={`w-3.5 h-3.5 ${isChecking ? 'animate-spin' : ''}`} 
                fill="none" 
                stroke="currentColor" 
                viewBox="0 0 24 24"
              >
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
              </svg>
              {isChecking ? '刷新中...' : '刷新'}
            </button>
          </div>
        </div>
        
        <div className="p-4">
          <div className="space-y-3">
            {/* Storage Permission */}
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <div className={`w-3 h-3 rounded-full ${permissionState.storage ? 'bg-green-500' : 'bg-red-500'}`} />
                <span className="text-sm text-gray-700">文件访问权限</span>
              </div>
              <button
                onClick={requestStoragePermission}
                className={`text-xs ${permissionState.storage ? 'text-green-600' : 'text-blue-500 hover:text-blue-600'}`}
              >
                {permissionState.storage ? '已授权' : '授权'}
              </button>
            </div>

            {/* Notification Permission */}
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <div className={`w-3 h-3 rounded-full ${permissionState.notification ? 'bg-green-500' : 'bg-red-500'}`} />
                <span className="text-sm text-gray-700">通知权限</span>
              </div>
              <button
                onClick={requestNotificationPermission}
                className={`text-xs ${permissionState.notification ? 'text-green-600' : 'text-blue-500 hover:text-blue-600'}`}
              >
                {permissionState.notification ? '已授权' : '授权'}
              </button>
            </div>

            {/* Battery Optimization */}
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <div className={`w-3 h-3 rounded-full ${permissionState.batteryOptimization ? 'bg-green-500' : 'bg-red-500'}`} />
                <span className="text-sm text-gray-700">电池优化白名单</span>
              </div>
              <button
                onClick={requestBatteryOptimization}
                className={`text-xs ${permissionState.batteryOptimization ? 'text-green-600' : 'text-blue-500 hover:text-blue-600'}`}
              >
                {permissionState.batteryOptimization ? '已授权' : '授权'}
              </button>
            </div>
          </div>
        </div>
      </Card>
    )}
  </>
  );
}
