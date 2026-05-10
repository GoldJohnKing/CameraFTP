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
import type { PresetLut } from '../types';

export const AutoLutConfigCard = memo(function AutoLutConfigCard() {
  const { isLoading, updateDraft } = useConfigStore();
  const draft = useDraftConfig();
  const [presetLuts, setPresetLuts] = useState<PresetLut[]>([]);

  useEffect(() => {
    invoke<PresetLut[]>('get_preset_luts')
      .then(setPresetLuts)
      .catch(() => {});
  }, []);

  if (!draft?.autoLut) return null;

  const options: SelectOption[] = presetLuts.map(p => ({
    value: p.id,
    label: p.displayName,
  }));

  const handleToggle = () => {
    updateDraft(d => ({
      ...d,
      autoLut: {
        ...d.autoLut!,
        enabled: !d.autoLut!.enabled,
      },
    }));
  };

  const handlePresetChange = (presetLutId: string) => {
    updateDraft(d => ({
      ...d,
      autoLut: {
        ...d.autoLut!,
        presetLutId,
      },
    }));
  };

  return (
    <Card>
      <CardHeader
        title="自动 LUT 滤镜"
        description="接收到 RAW 文件后自动应用 LUT 滤镜"
        icon={<Palette className="w-5 h-5 text-violet-600" />}
      />

      <div className="p-4 space-y-6">
        <ToggleSwitch
          enabled={draft.autoLut.enabled}
          onChange={handleToggle}
          label="自动应用 LUT 滤镜"
          description="RAW 文件上传后自动转为带胶片模拟滤镜的 JPEG"
          disabled={isLoading}
        />

        {draft.autoLut.enabled && (
          <div className="space-y-2">
            <label className="block text-sm font-medium text-gray-700">
              LUT 滤镜预设
            </label>
            <Select
              value={draft.autoLut.presetLutId}
              options={options}
              onChange={handlePresetChange}
              disabled={isLoading}
            />
            {!draft.autoLut.presetLutId && (
              <p className="text-xs text-red-500">请选择 LUT 预设</p>
            )}
          </div>
        )}
      </div>
    </Card>
  );
});
