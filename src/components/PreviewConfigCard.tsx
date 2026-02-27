import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { ImagePlay } from 'lucide-react';
import { Card, CardHeader } from './ui';
import type { PreviewWindowConfig } from '../types';

interface PreviewConfigCardProps {
  platform: string;
}

// 全局配置变化事件类型
interface ConfigChangedEvent {
  config: PreviewWindowConfig;
}

export function PreviewConfigCard({ platform }: PreviewConfigCardProps) {
  const isWindows = platform === 'windows';
  const [config, setConfig] = useState<PreviewWindowConfig | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [customPath, setCustomPath] = useState('');

  useEffect(() => {
    if (isWindows) {
      loadConfig();
    }
  }, [isWindows]);

  // 监听全局配置变化事件
  useEffect(() => {
    if (!isWindows) return;

    const setupListener = async () => {
      const unlisten = await listen<ConfigChangedEvent>('preview-config-changed', (event) => {
        setConfig(event.payload.config);
        if (event.payload.config.customPath) {
          setCustomPath(event.payload.config.customPath);
        }
      });
      return unlisten;
    };

    const unlistenPromise = setupListener();
    return () => {
      unlistenPromise.then(unlisten => unlisten());
    };
  }, [isWindows]);

  const loadConfig = async () => {
    try {
      const loaded = await invoke<PreviewWindowConfig>('get_preview_config');
      setConfig(loaded);
      // 恢复自定义路径显示
      if (loaded.customPath) {
        setCustomPath(loaded.customPath);
      }
    } catch (error) {
      console.error('Failed to load preview config:', error);
    } finally {
      setIsLoading(false);
    }
  };

  const updateConfig = async (updates: Partial<PreviewWindowConfig>) => {
    if (!config) return;

    const newConfig = { ...config, ...updates };
    setConfig(newConfig);

    try {
      await invoke('set_preview_config', { config: newConfig });
    } catch (error) {
      console.error('Failed to save preview config:', error);
    }
  };

  const handleSelectCustomProgram = async () => {
    try {
      const selected = await invoke<string | null>('select_executable_file');
      if (selected) {
        setCustomPath(selected);
        // 同时更新 method 和 customPath
        updateConfig({ method: 'custom', customPath: selected });
      }
    } catch (error) {
      console.error('Failed to select executable:', error);
    }
  };

  if (!isWindows) return null;

  return (
    <Card className="overflow-hidden">
      <CardHeader
        title="自动预览图片"
        description="相机上传图片后自动显示"
        icon={<ImagePlay className="w-5 h-5 text-purple-600" />}
      />

      <div className="p-4 space-y-6">
        {/* 总开关 */}
        <div className="flex items-center justify-between">
          <div>
            <h4 className="text-sm font-medium text-gray-700">启用自动预览</h4>
            <p className="text-xs text-gray-500 mt-0.5">
              接收到新图片时自动打开预览窗口
            </p>
          </div>
          <ToggleSwitch
            checked={config?.enabled ?? false}
            onChange={(checked) => updateConfig({ enabled: checked })}
            disabled={isLoading}
          />
        </div>

        {config?.enabled && (
          <>
            <hr className="border-gray-100" />

            {/* 打开方式选择 */}
            <div className="space-y-3">
              <h4 className="text-sm font-medium text-gray-700">打开方式</h4>

              <div className="space-y-2">
                <RadioOption
                  value="built-in-preview"
                  label="内置预览窗口"
                  description="独立窗口，支持全屏，自动显示最新图片"
                  selected={config.method === 'built-in-preview'}
                  onSelect={() => updateConfig({ method: 'built-in-preview' })}
                  recommended
                >
                  {config.method === 'built-in-preview' && (
                    <div className="pt-1.5 pb-1 border-t border-blue-200/50">
                      <div className="flex items-center justify-between">
                        <div>
                          <span className="text-sm text-gray-700">自动前台显示</span>
                          <p className="text-xs text-gray-500">接收到新图片后，自动将预览窗口置于前台</p>
                        </div>
                        <ToggleSwitch
                          checked={config.autoBringToFront}
                          onChange={(checked) => updateConfig({ autoBringToFront: checked })}
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
                    {customPath || '未设置'}
                  </p>
                </RadioOption>
              </div>
            </div>
          </>
        )}
      </div>
    </Card>
  );
}

// 辅助组件
function ToggleSwitch({ checked, onChange, disabled }: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
}) {
  return (
    <button
      onClick={() => onChange(!checked)}
      disabled={disabled}
      className={`
        relative inline-flex h-6 w-11 items-center rounded-full transition-colors
        ${checked ? 'bg-blue-600' : 'bg-gray-200'}
        ${disabled ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer'}
      `}
    >
      <span
        className={`
          inline-block h-4 w-4 transform rounded-full bg-white transition-transform
          ${checked ? 'translate-x-6' : 'translate-x-1'}
        `}
      />
    </button>
  );
}

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


