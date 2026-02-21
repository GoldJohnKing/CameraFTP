import { useEffect, useState, useCallback } from 'react';
import { Loader2 } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { toast } from 'sonner';
import { useConfigStore } from '../stores/configStore';
import { useStoragePermission } from '../hooks/useStoragePermission';
import { useSAFPicker } from '../hooks/useSAFPicker';

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

  const { pathInfo, isLoading: isLoadingPath, saveStoragePath, getLastUri } = useStoragePermission();
  const { openPicker } = useSAFPicker();

  const [autostartEnabled, setAutostartEnabled] = useState(false);
  const [isLoadingAutostart, setIsLoadingAutostart] = useState(false);
  const [portInput, setPortInput] = useState('2121');
  const [portError, setPortError] = useState<string | null>(null);
  const [isCheckingPort, setIsCheckingPort] = useState(false);

  // 是否是桌面平台（显示开机自启动选项）
  const isDesktop = platform === 'windows' || platform === 'macos' || platform === 'linux';

  const handleChangePath = useCallback(async () => {
    // Directly open SAF picker - no dialog
    const lastUri = await getLastUri();
    const result = await openPicker(lastUri || undefined);

    if (!result) {
      // User cancelled - no toast, just return
      return;
    }

    // Save new path
    const saved = await saveStoragePath(result.name, result.uri);

    if (saved) {
      toast.success(`存储路径已更新为：${result.name}`);
    } else {
      toast.error('更新存储路径失败');
    }
  }, [openPicker, saveStoragePath, getLastUri]);

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

  return (
    <div className="bg-white rounded-xl shadow-sm border border-gray-200 overflow-hidden">
      <div className="p-4 border-b border-gray-100">
        <h2 className="text-lg font-semibold text-gray-900">应用配置</h2>
        <p className="text-sm text-gray-500 mt-1">管理应用设置和偏好</p>
      </div>

      <div className="p-4 space-y-6">
        {/* 存储路径配置 */}
        <div className="bg-white rounded-lg p-4 shadow-sm">
          <h3 className="text-lg font-semibold mb-4">存储设置</h3>

          <div className="space-y-3">
            <div className="flex items-center justify-between">
              <span className="text-gray-600">当前路径</span>
              {isLoadingPath ? (
                <span className="text-gray-400">加载中...</span>
              ) : pathInfo ? (
                <span className="flex items-center gap-2">
                  <span className="text-gray-900">{pathInfo.path_name}</span>
                  {pathInfo.is_valid ? (
                    <span className="text-green-500">✅</span>
                  ) : (
                    <span className="text-red-500">❌ 权限失效</span>
                  )}
                </span>
              ) : (
                <span className="text-gray-400">未配置</span>
              )}
            </div>

            <button
              onClick={handleChangePath}
              disabled={isLoadingPath}
              className="w-full py-2 px-4 bg-blue-600 text-white rounded-lg hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
            >
              {pathInfo ? '更改存储路径' : '选择存储路径'}
            </button>
          </div>
        </div>

        {/* 开机自启动配置 - 仅在桌面平台显示 */}
        {isDesktop && (
          <div className="flex items-center justify-between py-2">
            <div>
              <label className="block text-sm font-medium text-gray-700">
                开机自启动
              </label>
              <p className="text-xs text-gray-500 mt-1">
                系统启动时自动运行图传伴侣
              </p>
            </div>
            <button
              onClick={handleAutostartToggle}
              disabled={isLoadingAutostart}
              className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
                autostartEnabled ? 'bg-blue-600' : 'bg-gray-200'
              } disabled:opacity-50 disabled:cursor-not-allowed`}
            >
              {isLoadingAutostart ? (
                <Loader2 className="w-4 h-4 animate-spin text-white absolute left-1" />
              ) : (
                <span
                  className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                    autostartEnabled ? 'translate-x-6' : 'translate-x-1'
                  }`}
                />
              )}
            </button>
          </div>
        )}

        {/* 端口配置 */}
        <div className="space-y-3">
          <div className="flex items-center justify-between py-2">
            <div>
              <label className="block text-sm font-medium text-gray-700">
                自动选择端口
              </label>
              <p className="text-xs text-gray-500 mt-1">
                自动寻找可用端口（推荐）
              </p>
            </div>
            <button
              onClick={handleAutoSelectToggle}
              disabled={isLoading}
              className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
                config?.auto_select_port ? 'bg-blue-600' : 'bg-gray-200'
              } disabled:opacity-50 disabled:cursor-not-allowed`}
            >
              <span
                className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                  config?.auto_select_port ? 'translate-x-6' : 'translate-x-1'
                }`}
              />
            </button>
          </div>

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
