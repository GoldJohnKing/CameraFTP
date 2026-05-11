/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useState, useEffect } from 'react';
import { Palette } from 'lucide-react';
import { Dialog } from './ui/Dialog';
import { Select } from './ui/Select';
import { ToggleSwitch } from './ui/ToggleSwitch';
import type { SelectOption } from './ui/Select';
import type { ColorGradingPreset } from '../types';
import { useConfigStore, useDraftConfig } from '../stores/configStore';

const METERING_MODES: SelectOption[] = [
  { value: 'highlight-safe', label: '高光保护' },
  { value: 'matrix', label: '矩阵测光' },
  { value: 'center-weighted', label: '中央重点测光' },
  { value: 'average', label: '平均测光' },
  { value: 'hybrid', label: '混合测光' },
];

interface ColorGradingDialogProps {
  isOpen: boolean;
  colorGradingPresets: ColorGradingPreset[];
  onConfirm: (lutId: string, useAutoExposure: boolean, meteringMode: string, manualEv: number) => void;
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
  const [useAutoExposure, setUseAutoExposure] = useState(true);
  const [meteringMode, setMeteringMode] = useState('highlight-safe');
  const [manualEv, setManualEv] = useState(0.0);
  const [syncToAuto, setSyncToAuto] = useState(false);

  useEffect(() => {
    if (isOpen) {
      const lastUsed = draft?.colorGradingLastUsed;
      const initialPreset = lastUsed?.presetId || colorGradingPresets[0]?.id || 'fujifilm-provia';
      setSelectedId(initialPreset);
      setUseAutoExposure(lastUsed?.useAutoExposure ?? true);
      setMeteringMode(lastUsed?.meteringMode ?? 'highlight-safe');
      setManualEv(lastUsed?.manualEv ?? 0.0);
      setSyncToAuto(false);
    }
  }, [isOpen, colorGradingPresets]);

  const handleConfirm = () => {
    if (!selectedId) return;

    updateDraft(d => ({
      ...d,
      colorGradingLastUsed: {
        presetId: selectedId,
        useAutoExposure,
        meteringMode,
        manualEv,
      },
      ...(syncToAuto && d.autoColorGrading ? {
        autoColorGrading: {
          ...d.autoColorGrading,
          presetId: selectedId,
          useAutoExposure,
          meteringMode,
          manualEv,
        },
      } : {}),
    }));

    onConfirm(selectedId, useAutoExposure, meteringMode, manualEv);
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
            <div className="flex items-center gap-2 cursor-pointer select-none" onClick={() => setSyncToAuto(!syncToAuto)}>
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

        <div className="border-t border-gray-100 pt-3">
          <ToggleSwitch
            enabled={useAutoExposure}
            onChange={setUseAutoExposure}
            label="自动曝光"
            description={useAutoExposure ? '自动检测并调整曝光' : '手动设置曝光补偿值'}
          />
        </div>

        {useAutoExposure && (
          <div>
            <label className="block text-sm font-medium text-gray-700 mb-1">测光模式</label>
            <Select
              value={meteringMode}
              options={METERING_MODES}
              onChange={setMeteringMode}
            />
          </div>
        )}

        {!useAutoExposure && (
          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <label className="block text-sm font-medium text-gray-700">曝光补偿</label>
              <span className="text-sm font-mono text-gray-500">{manualEv > 0 ? '+' : ''}{manualEv.toFixed(1)} EV</span>
            </div>
            <input
              type="range"
              min={-5.0}
              max={5.0}
              step={0.1}
              value={manualEv}
              onChange={(e) => setManualEv(parseFloat(e.target.value))}
              className="w-full h-2 bg-gray-200 rounded-lg appearance-none cursor-pointer accent-blue-600"
            />
            <div className="flex justify-between text-xs text-gray-400">
              <span>-5.0</span>
              <span>0</span>
              <span>+5.0</span>
            </div>
          </div>
        )}
      </div>
    </Dialog>
  );
}
