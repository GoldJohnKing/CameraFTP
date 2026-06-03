/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

// TODO: Extract Chinese UI strings for i18n when locale support is added

import { Select } from './ui/Select';
import { METERING_MODES } from '../constants/color-grading';

interface ExposureConfigSectionProps {
  meteringMode: string;
  onMeteringModeChange: (v: string) => void;
  evOffset: number;
  onEvOffsetChange: (v: number) => void;
  disabled?: boolean;
}

export function ExposureConfigSection({
  meteringMode,
  onMeteringModeChange,
  evOffset,
  onEvOffsetChange,
  disabled = false,
}: ExposureConfigSectionProps) {
  return (
    <>
      <div className="border-t border-gray-100 pt-3" />
      <div className="space-y-2">
        <label className="block text-sm font-medium text-gray-700">测光模式</label>
        <Select
          value={meteringMode}
          options={METERING_MODES}
          onChange={onMeteringModeChange}
          disabled={disabled}
        />
      </div>
      <div className="space-y-2">
        <div className="flex items-center justify-between">
          <label className="block text-sm font-medium text-gray-700">曝光偏移</label>
          <span className="text-sm font-mono text-gray-500">
            {evOffset > 0 ? '+' : ''}{evOffset.toFixed(1)} EV
          </span>
        </div>
        <input
          type="range"
          min={-5.0}
          max={5.0}
          step={0.1}
          value={evOffset}
          onChange={(e) => onEvOffsetChange(parseFloat(e.target.value))}
          disabled={disabled}
          className="w-full h-2 bg-gray-200 rounded-lg appearance-none cursor-pointer accent-blue-600 disabled:opacity-50"
        />
        <div className="flex justify-between text-xs text-gray-400">
          <span>-5.0</span>
          <span>0</span>
          <span>+5.0</span>
        </div>
      </div>
    </>
  );
}
