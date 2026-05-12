/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { memo, useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Palette } from 'lucide-react';
import { useConfigStore, useDraftConfig } from '../stores/configStore';
import { Card, CardHeader, ToggleSwitch } from './ui';
import { Select } from './ui/Select';
import type { SelectOption } from './ui/Select';
import type { ColorGradingPreset } from '../types';
import { METERING_MODES } from '../constants/color-grading';

export const AutoColorGradingConfigCard = memo(function AutoColorGradingConfigCard() {
  const { isLoading, updateDraft } = useConfigStore();
  const draft = useDraftConfig();
  const [colorGradingPresets, setColorGradingPresets] = useState<ColorGradingPreset[]>([]);

  useEffect(() => {
    invoke<ColorGradingPreset[]>('get_color_grading_presets')
      .then(setColorGradingPresets)
      .catch(() => {});
  }, []);

  if (!draft?.autoColorGrading) return null;

  const options: SelectOption[] = colorGradingPresets.map(p => ({
    value: p.id,
    label: p.displayName,
  }));

  const handleToggle = () => {
    updateDraft(d => ({
      ...d,
      autoColorGrading: {
        ...d.autoColorGrading!,
        enabled: !d.autoColorGrading!.enabled,
      },
    }));
  };

  const handlePresetChange = (presetId: string) => {
    updateDraft(d => ({
      ...d,
      autoColorGrading: {
        ...d.autoColorGrading!,
        presetId,
      },
    }));
  };

  const handleExposureToggle = () => {
    updateDraft(d => ({
      ...d,
      autoColorGrading: {
        ...d.autoColorGrading!,
        useAutoExposure: !d.autoColorGrading!.useAutoExposure,
      },
    }));
  };

  const handleMeteringModeChange = (meteringMode: string) => {
    updateDraft(d => ({
      ...d,
      autoColorGrading: {
        ...d.autoColorGrading!,
        meteringMode,
      },
    }));
  };

  const handleManualEvChange = (ev: number) => {
    updateDraft(d => ({
      ...d,
      autoColorGrading: {
        ...d.autoColorGrading!,
        manualEv: ev,
      },
    }));
  };

  return (
    <Card>
      <CardHeader
        title="自动调色"
        description="接收到 RAW 文件后自动应用调色"
        icon={<Palette className="w-5 h-5 text-violet-600" />}
      />

      <div className="p-4 space-y-6">
        <ToggleSwitch
          enabled={draft.autoColorGrading.enabled}
          onChange={handleToggle}
          label="自动调色"
          description="RAW 文件上传后自动转为带胶片模拟调色的 JPEG"
          disabled={isLoading}
        />

        {draft.autoColorGrading.enabled && (
          <div className="space-y-4">
            <div className="space-y-2">
              <label className="block text-sm font-medium text-gray-700">
                调色预设
              </label>
              <Select
                value={draft.autoColorGrading.presetId}
                options={options}
                onChange={handlePresetChange}
                disabled={isLoading}
              />
              {!draft.autoColorGrading.presetId && (
                <p className="text-xs text-red-500">请选择调色预设</p>
              )}
            </div>

            <div className="border-t border-gray-100 pt-4">
              <ToggleSwitch
                enabled={draft.autoColorGrading.useAutoExposure}
                onChange={handleExposureToggle}
                label="自动曝光"
                description={draft.autoColorGrading.useAutoExposure ? '自动检测并调整曝光' : '手动设置曝光补偿值'}
                disabled={isLoading}
              />
            </div>

            {draft.autoColorGrading.useAutoExposure && (
              <div className="space-y-2">
                <label className="block text-sm font-medium text-gray-700">测光模式</label>
                <Select
                  value={draft.autoColorGrading.meteringMode}
                  options={METERING_MODES}
                  onChange={handleMeteringModeChange}
                  disabled={isLoading}
                />
              </div>
            )}

            {!draft.autoColorGrading.useAutoExposure && (
              <div className="space-y-2">
                <div className="flex items-center justify-between">
                  <label className="block text-sm font-medium text-gray-700">曝光补偿</label>
                  <span className="text-sm font-mono text-gray-500">
                    {draft.autoColorGrading.manualEv > 0 ? '+' : ''}{draft.autoColorGrading.manualEv.toFixed(1)} EV
                  </span>
                </div>
                <input
                  type="range"
                  min={-5.0}
                  max={5.0}
                  step={0.1}
                  value={draft.autoColorGrading.manualEv}
                  onChange={(e) => handleManualEvChange(parseFloat(e.target.value))}
                  disabled={isLoading}
                  className="w-full h-2 bg-gray-200 rounded-lg appearance-none cursor-pointer accent-blue-600 disabled:opacity-50"
                />
                <div className="flex justify-between text-xs text-gray-400">
                  <span>-5.0</span>
                  <span>0</span>
                  <span>+5.0</span>
                </div>
              </div>
            )}
          </div>
        )}
      </div>
    </Card>
  );
});
