import { useEffect, useState } from 'react';
import { Loader2, Folder, Settings, CheckCircle, AlertCircle, RefreshCw } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { useConfigStore } from '../stores/configStore';
import { useStoragePermission } from '../hooks/useStoragePermission';
import { ToggleSwitch } from './ui/ToggleSwitch';

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
    isReady,
    displayName,
    loadStorageInfo,
    checkPermissionStatus,
    requestAllFilesPermission,
    ensureStorageReady,
  } = useStoragePermission();

  const [autostartEnabled, setAutostartEnabled] = useState(false);
  const [isLoadingAutostart, setIsLoadingAutostart] = useState(false);
  const [portInput, setPortInput] = useState('2121');
  const [portError, setPortError] = useState<string | null>(null);
  const [isCheckingPort, setIsCheckingPort] = useState(false);
  const [isCreatingDir, setIsCreatingDir] = useState(false);

  // 是否是桌面平台（显示开机自启动选项）
  const isDesktop = platform === 'windows' || platform === 'macos' || platform === 'linux';
  const isAndroid = platform === 'android';

  useEffect(() => {
    loadConfig();
    loadPlatform();
    loadAutostartStatus();
  }, []);

  // 当配置加载后，同步端口输入值
  useEffect(() => {
    if (config) {
      setPortInput(config.port.toString());
    }
  }, [config?.port]);

  const loadAutostartStatus = async () => {
    try {
      const status = await invoke<boolean>('get_autostart_status');
      setAutostartEnabled(status);
    } catch (err) {
      console.error('Failed to load autostart status:', err);
    }
  };

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
    } catch (err) {
      console.error('Failed to check port:', err);
      setPortError('检查端口时出错');
    } finally {
      setIsCheckingPort(false);
    }
  };

  const handleRequestPermission = async () => {
    await requestAllFilesPermission();
  };

  const handleEnsureReady = async () => {
    setIsCreatingDir(true);
    try {
      await ensureStorageReady();
    } finally {
      setIsCreatingDir(false);
    }
  };

  const handleRefreshStorage = async () => {
    await loadStorageInfo();
    await checkPermissionStatus();
  };

  return (
    <div className="bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
      <div className="p-4 border-b border-gray-100">
        <h2 className="text-lg font-semibold text-gray-900">应用配置</h2>
        <p className="text-sm text-gray-500 mt-1">管理应用设置和偏好</p>
      </div>

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
                  <div className="flex items-center gap-1 mt-1">
                    <span className="text-xs text-gray-500 truncate">{displayName}</span>
                    {isReady ? (
                      <CheckCircle className="w-3.5 h-3.5 text-green-500 shrink-0" />
                    ) : needsPermission ? (
                      <AlertCircle className="w-3.5 h-3.5 text-yellow-500 shrink-0" />
                    ) : (
                      <AlertCircle className="w-3.5 h-3.5 text-red-500 shrink-0" />
                    )}
                  </div>
                )}
              </div>
              <button
                onClick={handleRefreshStorage}
                disabled={isLoadingStorage}
                className="ml-3 shrink-0 p-1.5 text-gray-400 hover:text-gray-600 rounded-lg hover:bg-gray-100 disabled:opacity-50 transition-colors"
                title="刷新状态"
              >
                <RefreshCw className={`w-4 h-4 ${isLoadingStorage ? 'animate-spin' : ''}`} />
              </button>
            </div>

            {/* 权限提示 */}
            {needsPermission && (
              <div className="bg-yellow-50 border border-yellow-200 rounded-lg p-3">
                <div className="flex items-start gap-2">
                  <AlertCircle className="w-4 h-4 text-yellow-600 flex-shrink-0 mt-0.5" />
                  <div className="flex-1 min-w-0">
                    <p className="text-xs font-medium text-yellow-800">需要存储权限</p>
                    <p className="text-xs text-yellow-700 mt-0.5">
                      应用需要"所有文件访问权限"才能接收照片
                    </p>
                    <button
                      onClick={handleRequestPermission}
                      className="mt-2 flex items-center gap-1 px-2 py-1 bg-yellow-600 text-white text-xs rounded hover:bg-yellow-700 transition-colors"
                    >
                      <Settings className="w-3 h-3" />
                      前往授权
                    </button>
                  </div>
                </div>
              </div>
            )}

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

        {/* 开机自启动配置 - 仅在桌面平台显示，使用 ToggleSwitch 组件 */}
        {isDesktop && (
          <ToggleSwitch
            enabled={autostartEnabled}
            onChange={handleAutostartToggle}
            label="开机自启动"
            description="系统启动时自动运行图传伴侣"
            disabled={isLoadingAutostart}
          />
        )}

        {/* 端口配置 - 使用 ToggleSwitch 组件 */}
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
    </div>
  );
}
