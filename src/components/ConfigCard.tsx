/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useEffect, useState, useCallback, memo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Settings, Wifi, Shield, Image } from 'lucide-react';
import { useConfigStore, useDraftConfig } from '../stores/configStore';
import { usePermissionStore } from '../stores/permissionStore';
import { useServerStore } from '../stores/serverStore';
import { Card, CardHeader, ToggleSwitch } from './ui';
import { PermissionList } from './PermissionList';
import { PathSelector } from './PathSelector';
import { AdvancedConnectionConfigPanel } from './AdvancedConnectionConfig';
import { PreviewConfigCard } from './PreviewConfigCard';
import { AboutCard } from './AboutCard';
import type { AdvancedConnectionConfig, AppConfig } from '../types';

const DEFAULT_ADVANCED_CONFIG: AdvancedConnectionConfig = {
  enabled: false,
  auth: { anonymous: true, username: '', passwordHash: '' },
};

export const ConfigCard = memo(function ConfigCard() {
  const {
    isLoading,
    error,
    platform,
    setAutostart,
    updateDraft,
  } = useConfigStore();

  // 使用 draft（编辑界面订阅 draft，而非 config）
  const draft = useDraftConfig();

  const {
    storageInfo,
    needsPermission,
    ensureStorageReady,
    checkPermissions,
  } = usePermissionStore();

  const { isRunning } = useServerStore();

  const [autostartEnabled, setAutostartEnabled] = useState(false);
  const [isCheckingPermissions, setIsCheckingPermissions] = useState(false);

  // Platform detection
  const isDesktop = platform === 'windows';
  const isAndroid = platform === 'android';

  useEffect(() => {
    const isCancelled = { current: false };

    const loadAutostartStatus = async () => {
      try {
        const status = await invoke<boolean>('get_autostart_status');
        if (!isCancelled.current) {
          setAutostartEnabled(status);
        }
      } catch {
        // Silently ignore autostart status load errors
      }
    };

    void loadAutostartStatus();

    return () => {
      isCancelled.current = true;
    };
  }, []);

  const handleAutostartToggle = async () => {
    const newValue = !autostartEnabled;
    // 乐观更新：立即反映 UI 变化
    setAutostartEnabled(newValue);
    try {
      await setAutostart(newValue);
    } catch {
      // 失败时回滚
      setAutostartEnabled(!newValue);
    }
  };

  const handleRefreshPermissions = useCallback(async () => {
    setIsCheckingPermissions(true);
    const startTime = Date.now();
    
    try {
      await checkPermissions();
    } finally {
      // 确保动画至少持续 200ms，让用户能看到刷新效果
      const elapsed = Date.now() - startTime;
      const minDuration = 200;
      const remaining = Math.max(0, minDuration - elapsed);
      
      setTimeout(() => {
        setIsCheckingPermissions(false);
      }, remaining);
    }
  }, [checkPermissions]);

  const handleSelectDirectory = async () => {
    const result = await invoke<string | null>('select_save_directory');
    if (result && draft) {
      // 直接更新 draft（触发防抖保存）
      updateDraft(d => ({ ...d, savePath: result }));
    }
  };

  // 处理高级连接配置更新
  const handleAdvancedConfigUpdate = useCallback((updater: (draft: AppConfig) => Partial<AppConfig>) => {
    updateDraft(draft => {
      const updates = updater(draft);
      return { ...draft, ...updates };
    });
  }, [updateDraft]);

  return (
    <div className="space-y-4">
      {/* 基础设置 - Android上增加顶部留白 */}
      <Card className={`overflow-hidden ${isAndroid ? 'mt-6' : ''}`}>
        <CardHeader 
          title="基础设置" 
          description="管理应用的基础设置和偏好"
          icon={<Settings className="w-5 h-5 text-cyan-600" />}
        />

        <div className="p-4 space-y-6">
          {/* 路径选择 */}
          <PathSelector
            platform={platform}
            storageInfo={storageInfo}
            needsPermission={needsPermission}
            savePath={draft?.savePath ?? null}
            isLoading={isLoading}
            disabled={isRunning}
            ensureStorageReady={ensureStorageReady}
            onSelectDirectory={handleSelectDirectory}
          />

          {/* 开机自启动配置 - 仅在桌面平台显示 */}
          {isDesktop && (
            <ToggleSwitch
              enabled={autostartEnabled}
              onChange={handleAutostartToggle}
              label="开机自启动"
              description="系统启动时自动运行图传伴侣"
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
          title="高级连接设置"
          description="自定义 FTP 服务器连接参数"
          icon={<Wifi className="w-5 h-5 text-indigo-600" />}
          action={
            <ToggleSwitch
              ariaLabel="启用高级连接设置"
              enabled={draft?.advancedConnection?.enabled ?? false}
              onChange={(enabled) => {
                const currentConfig = draft?.advancedConnection ?? DEFAULT_ADVANCED_CONFIG;
                handleAdvancedConfigUpdate(() => ({
                  advancedConnection: {
                    ...currentConfig,
                    enabled,
                  },
                }));
              }}
              disabled={isLoading || isRunning}
            />
          }
        />

        {draft?.advancedConnection?.enabled && (
          <div className="p-4 space-y-6">
            {/* 高级连接配置 */}
            <AdvancedConnectionConfigPanel
              config={draft.advancedConnection}
              port={draft?.port ?? 2121}
              platform={platform}
              isLoading={isLoading}
              disabled={isRunning}
              onUpdate={handleAdvancedConfigUpdate}
            />
          </div>
        )}
      </Card>

      {/* 预览配置卡片（Windows 专属） */}
      <PreviewConfigCard platform={platform} />

      {/* 图片查看设置（Android 专属） */}
      {isAndroid && draft?.androidImageViewer && (
        <Card className="overflow-hidden">
          <CardHeader
            title="图片查看设置"
            description="配置图片查看相关选项"
            icon={<Image className="w-5 h-5 text-violet-600" />}
          />
          <div className="p-4 space-y-4">
            <ToggleSwitch
              label="使用外部应用打开图片"
              description="使用第三方APP打开图片"
              enabled={draft.androidImageViewer.openMethod === 'external-app'}
              onChange={(enabled) => {
                updateDraft(d => ({
                  ...d,
                  androidImageViewer: {
                    ...d.androidImageViewer!,
                    openMethod: enabled ? 'external-app' : 'built-in-viewer',
                  },
                }));
              }}
              disabled={isLoading}
            />

            {draft.androidImageViewer.openMethod !== 'external-app' && (
              <ToggleSwitch
                label="自动预览"
                description="收到新图片后自动显示预览"
                enabled={draft.androidImageViewer.autoOpenLatestWhenVisible}
                onChange={(enabled) => {
                  updateDraft(d => ({
                    ...d,
                    androidImageViewer: {
                      ...d.androidImageViewer!,
                      autoOpenLatestWhenVisible: enabled,
                    },
                  }));
                }}
                disabled={isLoading}
              />
            )}
          </div>
        </Card>
      )}

      {/* 权限状态 - Android 特有，放在最后 */}
      {isAndroid && typeof window !== 'undefined' && window.PermissionAndroid && (
        <Card className="overflow-hidden">
          <CardHeader
            title="权限状态"
            description="管理应用所需权限"
            icon={<Shield className="w-5 h-5 text-emerald-600" />}
            action={
              <button
                onClick={handleRefreshPermissions}
                disabled={isCheckingPermissions}
                className="text-sm text-blue-500 hover:text-blue-600 flex items-center gap-1.5 disabled:opacity-50 transition-colors"
              >
                <svg
                  className={`w-4 h-4 ${isCheckingPermissions ? 'animate-spin' : ''}`}
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                >
                  <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
                </svg>
                <span>{isCheckingPermissions ? '刷新中...' : '刷新'}</span>
              </button>
            }
          />

          <div className="p-4 space-y-4">
            <PermissionList variant="compact" />
          </div>
        </Card>
      )}

      {/* 关于 - 放在配置页面底部 */}
      <AboutCard />
    </div>
  );
});
