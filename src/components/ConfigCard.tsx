import { useEffect, useState, useCallback, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Settings, Wifi } from 'lucide-react';
import { useConfigStore } from '../stores/configStore';
import { usePermissionStore } from '../stores/permissionStore';
import { useServerStore } from '../stores/serverStore';
import { Card, CardHeader } from './ui';
import { PermissionList } from './PermissionList';
import { PathSelector } from './PathSelector';
import { PortSelector } from './PortSelector';
import { AutoStartToggle } from './AutoStartToggle';
import { PreviewConfigCard } from './PreviewConfigCard';

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
    needsPermission,
    ensureStorageReady,
    checkPermissions,
  } = usePermissionStore();

  const { isRunning } = useServerStore();

  const [autostartEnabled, setAutostartEnabled] = useState(false);
  const [isLoadingAutostart, setIsLoadingAutostart] = useState(false);
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

  // Track timeout for cleanup
  const checkingTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Wrapper for checkPermissions with loading state
  const handleCheckPermissions = useCallback(async () => {
    setIsChecking(true);
    try {
      await checkPermissions();
    } finally {
      checkingTimeoutRef.current = setTimeout(() => setIsChecking(false), 300);
    }
  }, [checkPermissions]);

  // Cleanup timeout on unmount
  useEffect(() => {
    return () => {
      if (checkingTimeoutRef.current) {
        clearTimeout(checkingTimeoutRef.current);
      }
    };
  }, []);

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

  const handleSelectDirectory = async () => {
    const result = await invoke<string | null>('select_save_directory');
    if (result && config) {
      const newConfig = { ...config, save_path: result };
      await invoke('save_config', { config: newConfig });
      loadConfig();
    }
  };

  return (
    <>
      {/* 通用配置 */}
      <Card className="overflow-hidden">
        <CardHeader 
          title="通用配置" 
          description="管理应用设置和偏好"
          icon={<Settings className="w-5 h-5 text-cyan-600" />}
        />

        <div className="p-4 space-y-6">
          {/* 路径选择 */}
          <PathSelector
            platform={platform}
            storageInfo={storageInfo}
            needsPermission={needsPermission}
            savePath={config?.save_path ?? null}
            isLoading={isLoading}
            disabled={isRunning}
            ensureStorageReady={ensureStorageReady}
            onSelectDirectory={handleSelectDirectory}
          />

          {/* 开机自启动配置 - 仅在桌面平台显示 */}
          {isDesktop && (
            <AutoStartToggle
              enabled={autostartEnabled}
              isLoading={isLoadingAutostart}
              onToggle={handleAutostartToggle}
            />
          )}

          {/* 错误提示 */}
          {error && (
            <div className="p-3 bg-red-50 border border-red-200 rounded-lg">
              <p className="text-sm text-red-600">{error}</p>
            </div>
          )}
        </div>
      </Card>

      {/* 连接设置 */}
      <Card className="overflow-hidden">
        <CardHeader 
          title="连接设置" 
          description="配置 FTP 服务器连接参数"
          icon={<Wifi className="w-5 h-5 text-indigo-600" />}
        />

        <div className="p-4 space-y-6">
          {/* 端口配置 */}
          <PortSelector
            autoSelectPort={config?.auto_select_port ?? true}
            port={config?.port ?? 2121}
            isLoading={isLoading}
            disabled={isRunning}
            onAutoSelectToggle={updateAutoSelectPort}
            onPortChange={updatePort}
          />

          {isRunning && (
            <div className="p-3 bg-amber-50 border border-amber-200 rounded-lg">
              <p className="text-sm text-amber-700">
                服务器正在运行，部分设置已禁用。停止服务器后可修改。
              </p>
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
            <PermissionList variant="compact" />
          </div>
        </Card>
      )}

      {/* 预览配置卡片（Windows 专属） */}
      <PreviewConfigCard platform={platform} />
    </>
  );
}
