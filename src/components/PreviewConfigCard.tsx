import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Card, CardHeader } from './ui';
import type { PreviewWindowConfig } from '../types';

interface PreviewConfigCardProps {
  platform: string;
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
        title="🖼️ 自动预览图片"
        description="Windows 专属功能：相机上传图片后自动显示"
      />

      <div className="p-4 space-y-6">
        {/* 总开关 */}
        <div className="flex items-center justify-between">
          <div>
            <h4 className="text-sm font-medium text-gray-700">启用自动预览</h4>
            <p className="text-xs text-gray-500 mt-0.5">
              新图片到达时自动打开预览窗口
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
                  description="独立窗口，支持全屏，单窗口始终显示最新图片"
                  selected={config.method === 'built-in-preview'}
                  onSelect={() => updateConfig({ method: 'built-in-preview' })}
                  recommended
                />

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
                  description={customPath || '选择其他程序打开图片'}
                  selected={config.method === 'custom'}
                  onSelect={() => updateConfig({ method: 'custom' })}
                />
              </div>

              {config.method === 'custom' && (
                <button
                  onClick={handleSelectCustomProgram}
                  className="mt-2 w-full text-left px-3 py-2 text-sm border border-gray-300 rounded-lg hover:border-blue-400 hover:bg-blue-50 transition-colors"
                >
                  {customPath || '点击选择程序...'}
                </button>
              )}
            </div>

            {/* 通用设置 */}
            <hr className="border-gray-100" />

            <div className="space-y-3">
              <h4 className="text-sm font-medium text-gray-700">通用设置</h4>

              <Checkbox
                checked={config.autoBringToFront}
                onChange={(checked) => updateConfig({ autoBringToFront: checked })}
                label="新图片时自动前台显示"
                description="打开图片后将窗口置于最前"
              />
            </div>

            {/* 内置预览专属设置 */}
            {config.method === 'built-in-preview' && (
              <>
                <hr className="border-gray-100" />

                <div className="space-y-3">
                  <h4 className="text-sm font-medium text-gray-700">预览窗口设置</h4>

                  <Checkbox
                    checked={config.rememberPosition}
                    onChange={(checked) => updateConfig({ rememberPosition: checked })}
                    label="记住窗口位置和大小"
                    description="下次打开时恢复到上次关闭的状态"
                  />
                </div>
              </>
            )}
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
  recommended
}: {
  value: string;
  label: string;
  description: string;
  selected: boolean;
  onSelect: () => void;
  recommended?: boolean;
}) {
  return (
    <label 
      className={`
        flex items-start gap-3 p-3 rounded-lg border cursor-pointer transition-colors
        ${selected 
          ? 'border-blue-400 bg-blue-50' 
          : 'border-gray-200 hover:border-blue-300 hover:bg-blue-50/50'
        }
      `}
    >
      <input
        type="radio"
        name="open-method"
        value={value}
        checked={selected}
        onChange={onSelect}
        className="mt-0.5"
      />
      <div className="flex-1">
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium text-gray-700">{label}</span>
          {recommended && (
            <span className="text-xs px-1.5 py-0.5 bg-blue-100 text-blue-700 rounded">
              推荐
            </span>
          )}
        </div>
        <p className="text-xs text-gray-500 mt-0.5">{description}</p>
      </div>
    </label>
  );
}

function Checkbox({ checked, onChange, label, description }: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: string;
  description: string;
}) {
  return (
    <label className="flex items-start gap-3 cursor-pointer">
      <input
        type="checkbox"
        checked={checked}
        onChange={(e) => onChange(e.target.checked)}
        className="mt-0.5 rounded border-gray-300 text-blue-600 focus:ring-blue-500"
      />
      <div>
        <span className="text-sm text-gray-700">{label}</span>
        <p className="text-xs text-gray-500">{description}</p>
      </div>
    </label>
  );
}
