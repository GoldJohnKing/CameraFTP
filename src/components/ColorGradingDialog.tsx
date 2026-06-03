/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

// TODO: Extract Chinese UI strings for i18n when locale support is added

import { useState, useEffect } from 'react';
import { Palette } from 'lucide-react';
import { Dialog } from './ui/Dialog';
import { Select } from './ui/Select';
import { ToggleSwitch } from './ui/ToggleSwitch';
import type { SelectOption } from './ui/Select';
import type { ColorGradingPreset } from '../types';
import { useConfigStore, useDraftConfig } from '../stores/configStore';
import { ExposureConfigSection } from './ExposureConfigSection';

interface ColorGradingDialogProps {
  isOpen: boolean;
  colorGradingPresets: ColorGradingPreset[];
  onConfirm: (lutId: string, meteringMode: string, evOffset: number) => void;
  onCancel: () => void;
}

export function ColorGradingDialog({ isOpen, colorGradingPresets, onConfirm, onCancel }: ColorGradingDialogProps) {
  const options: SelectOption[] = colorGradingPresets.map(p => ({
    value: p.id,
    label: p.displayName,
  }));

  const { updateDraft } = useConfigStore();
  const draft = useDraftConfig();

  const [selectedId, setSelectedId] = useState('');
  const [meteringMode, setMeteringMode] = useState('highlight-safe');
  const [evOffset, setEvOffset] = useState(0.0);
  const [syncToAuto, setSyncToAuto] = useState(false);

  useEffect(() => {
    if (isOpen) {
      const lastUsed = draft?.colorGradingLastUsed;
      const initialPreset = lastUsed?.presetId || colorGradingPresets[0]?.id || 'fujifilm-provia';
      setSelectedId(initialPreset);
      setMeteringMode(lastUsed?.meteringMode ?? 'highlight-safe');
      setEvOffset(lastUsed?.evOffset ?? 0.0);
      setSyncToAuto(false);
    }
  // draft intentionally excluded — effect should only run on mount/dialog open
  }, [isOpen, colorGradingPresets]);

  const handleConfirm = () => {
    if (!selectedId) return;

    updateDraft(d => ({
      ...d,
      colorGradingLastUsed: {
        presetId: selectedId,
        meteringMode,
        evOffset,
      },
      ...(syncToAuto && d.autoColorGrading ? {
        autoColorGrading: {
          ...d.autoColorGrading,
          presetId: selectedId,
          meteringMode,
          evOffset,
        },
      } : {}),
    }));

    onConfirm(selectedId, meteringMode, evOffset);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && selectedId) {
      e.preventDefault();
      handleConfirm();
    }
  };

  const autoColorGradingEnabled = draft?.autoColorGrading?.enabled ?? false;

  return (
    <Dialog
      isOpen={isOpen}
      onClose={onCancel}
      title="调色"
      subtitle="使用胶片模拟调色处理 RAW 照片"
      icon={
        <div className="w-10 h-10 bg-gray-100 rounded-lg flex items-center justify-center">
          <Palette className="w-5 h-5 text-violet-600" />
        </div>
      }
      footer={
        <div className="flex items-center justify-between w-full">
          {autoColorGradingEnabled ? (
            <div className="flex items-center gap-2 cursor-pointer select-none">
              <ToggleSwitch enabled={syncToAuto} onChange={setSyncToAuto} />
              <span className="text-sm font-medium text-gray-700">同步到自动调色</span>
            </div>
          ) : (
            <div />
          )}
          <div className="flex gap-2">
            <button
              onClick={onCancel}
              className="px-4 py-2 text-gray-700 bg-gray-100 rounded-lg hover:bg-gray-200 transition-colors text-sm"
            >
              取消
            </button>
            <button
              onClick={handleConfirm}
              disabled={!selectedId}
              className="px-4 py-2 text-white bg-blue-600 rounded-lg hover:bg-blue-700 transition-colors text-sm disabled:opacity-50 disabled:cursor-not-allowed"
            >
              应用
            </button>
          </div>
        </div>
      }
    >
      <div className="space-y-3" onKeyDown={handleKeyDown}>
        <div>
          <label className="block text-sm font-medium text-gray-700 mb-1">调色预设</label>
          <Select
            value={selectedId}
            options={options}
            onChange={setSelectedId}
          />
        </div>

        <ExposureConfigSection
          meteringMode={meteringMode}
          onMeteringModeChange={setMeteringMode}
          evOffset={evOffset}
          onEvOffsetChange={setEvOffset}
        />
      </div>
    </Dialog>
  );
}
