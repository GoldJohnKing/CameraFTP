/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

// TODO: Extract Chinese UI strings for i18n when locale support is added

import { memo } from 'react';
import { Palette } from 'lucide-react';
import { useConfigStore, useDraftConfig } from '../stores/configStore';
import { Card, CardHeader, ToggleSwitch } from './ui';
import { Select } from './ui/Select';
import type { SelectOption } from './ui/Select';
import { useColorGradingPresets } from '../hooks/useColorGradingPresets';
import { ExposureConfigSection } from './ExposureConfigSection';

export const AutoColorGradingConfigCard = memo(function AutoColorGradingConfigCard() {
  const { isLoading, updateDraft } = useConfigStore();
  const draft = useDraftConfig();
  const colorGradingPresets = useColorGradingPresets();

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

  const handleMeteringModeChange = (meteringMode: string) => {
    updateDraft(d => ({
      ...d,
      autoColorGrading: {
        ...d.autoColorGrading!,
        meteringMode,
      },
    }));
  };

  const handleEvOffsetChange = (ev: number) => {
    updateDraft(d => ({
      ...d,
      autoColorGrading: {
        ...d.autoColorGrading!,
        evOffset: ev,
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

            <ExposureConfigSection
              meteringMode={draft.autoColorGrading.meteringMode}
              onMeteringModeChange={handleMeteringModeChange}
              evOffset={draft.autoColorGrading.evOffset}
              onEvOffsetChange={handleEvOffsetChange}
              disabled={isLoading}
            />
          </div>
        )}
      </div>
    </Card>
  );
});
