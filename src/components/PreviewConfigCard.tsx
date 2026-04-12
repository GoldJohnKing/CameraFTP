/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { PreviewWindowConfig } from '../types';
import { ImagePlay } from 'lucide-react';
import { Card, CardHeader, ToggleSwitch } from './ui';
import { useConfigStore } from '../stores/configStore';
import { usePreviewConfigListener } from '../hooks/usePreviewConfigListener';
import { usePlatform } from '../hooks/usePlatform';

export function PreviewConfigCard() {
  const { isWindows } = usePlatform();
  const updatePreviewConfig = useConfigStore(state => state.updatePreviewConfig);
  const applyPreviewConfig = useConfigStore(state => state.applyPreviewConfig);
  const config = useConfigStore(state => state.draft?.previewConfig ?? null);
  const isLoading = useConfigStore(state => state.isLoading || state.draft == null);

  usePreviewConfigListener(
    useCallback((config) => applyPreviewConfig(config), [applyPreviewConfig]),
    isWindows,
  );

  const updateConfig = async (updates: Partial<PreviewWindowConfig>) => {
    if (!config) return;

    try {
      await updatePreviewConfig(updates);
    } catch {
      // Silently ignore - config change is not critical
    }
  };

  const handleSelectCustomProgram = async () => {
    try {
      const selected = await invoke<string | null>('select_executable_file');
      if (selected) {
        await updateConfig({ method: 'custom', customPath: selected });
      }
    } catch {
      // Silently ignore - user can try again
    }
  };

  if (!isWindows) return null;

  return (
    <Card className="overflow-hidden">
      <CardHeader
        title="自动预览"
        description="收到新图片后自动显示预览"
        icon={<ImagePlay className="w-5 h-5 text-purple-600" />}
        action={
          <ToggleSwitch
            enabled={config?.enabled ?? false}
            onChange={(enabled) => updateConfig({ enabled })}
            disabled={isLoading}
          />
        }
      />

      {config?.enabled && (
        <div className="p-4 space-y-6">
          {/* 打开方式选择 */}
          <div className="space-y-3">
            <h4 className="text-sm font-semibold text-gray-800">打开方式</h4>

            <div className="space-y-2">
              <RadioOption
                value="built-in-preview"
                label="内置预览窗口"
                description="自动显示最新图片，支持显示拍摄参数等"
                selected={config.method === 'built-in-preview'}
                onSelect={() => updateConfig({ method: 'built-in-preview' })}
                recommended
              >
                {config.method === 'built-in-preview' && (
                  <div className="mt-2 flex flex-col gap-2">
                    <div className="h-px bg-gray-200" />
                    <div className="flex items-center justify-between">
                      <div>
                        <span className="text-sm text-gray-700">自动置顶</span>
                        <p className="text-xs text-gray-500">接收到新图片时预览窗口自动置顶</p>
                      </div>
                      <ToggleSwitch
                        enabled={config.autoBringToFront}
                        onChange={(enabled) => updateConfig({ autoBringToFront: enabled })}
                        disabled={isLoading}
                      />
                    </div>
                  </div>
                )}
              </RadioOption>

              <RadioOption
                value="system-default"
                label="系统默认程序"
                description="使用 Windows 默认的图片查看器"
                selected={config.method === 'system-default'}
                onSelect={() => updateConfig({ method: 'system-default' })}
              />

              <RadioOption
                value="windows-photos"
                label="Microsoft 照片应用"
                description="Windows 自带的照片应用"
                selected={config.method === 'windows-photos'}
                onSelect={() => updateConfig({ method: 'windows-photos' })}
              />

              <RadioOption
                value="custom"
                label="自定义程序"
                selected={config.method === 'custom'}
                onSelect={() => updateConfig({ method: 'custom' })}
                action={
                  <button
                    onClick={(e) => {
                      e.preventDefault();
                      e.stopPropagation();
                      handleSelectCustomProgram();
                    }}
                    className="shrink-0 px-3 py-1.5 text-sm bg-blue-600 text-white rounded-lg hover:bg-blue-700 transition-colors"
                  >
                    更改
                  </button>
                }
              >
                <p className={`text-xs truncate ${config.method === 'custom' ? 'text-gray-500' : 'text-gray-400'}`}>
                  {config.customPath || '未设置'}
                </p>
              </RadioOption>
            </div>
          </div>
        </div>
      )}
    </Card>
  );
}

// 辅助组件
function RadioOption({
  value,
  label,
  description,
  selected,
  onSelect,
  recommended,
  children,
  action
}: {
  value: string;
  label: string;
  description?: string;
  selected: boolean;
  onSelect: () => void;
  recommended?: boolean;
  children?: React.ReactNode;
  action?: React.ReactNode;
}) {
  return (
    <label 
      className={`
        flex flex-col gap-2 p-3 rounded-lg border cursor-pointer transition-colors
        ${selected 
          ? 'border-blue-400 bg-blue-50' 
          : 'border-gray-200 hover:border-blue-300 hover:bg-blue-50/50'
        }
      `}
    >
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-start gap-3 flex-1 min-w-0">
          <input
            type="radio"
            name="open-method"
            value={value}
            checked={selected}
            onChange={onSelect}
            className="mt-0.5 shrink-0"
          />
          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-2 flex-wrap">
              <span className="text-sm font-medium text-gray-700">{label}</span>
              {recommended && (
                <span className="text-xs px-1.5 py-0.5 bg-blue-100 text-blue-700 rounded">
                  推荐
                </span>
              )}
            </div>
            {description && <p className="text-xs text-gray-500 mt-0.5 truncate">{description}</p>}
            {children}
          </div>
        </div>
        {action && <div className="flex items-center shrink-0">{action}</div>}
      </div>
    </label>
  );
}
