/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useState, useEffect } from 'react';
import { Palette } from 'lucide-react';
import { Dialog } from './ui/Dialog';
import { Select } from './ui/Select';
import type { SelectOption } from './ui/Select';
import type { PresetLut } from '../types';

interface LutFilterDialogProps {
  isOpen: boolean;
  presetLuts: PresetLut[];
  onConfirm: (lutId: string) => void;
  onCancel: () => void;
}

export function LutFilterDialog({ isOpen, presetLuts, onConfirm, onCancel }: LutFilterDialogProps) {
  const options: SelectOption[] = presetLuts.map(p => ({
    value: p.id,
    label: p.displayName,
  }));

  const [selectedId, setSelectedId] = useState(presetLuts[0]?.id ?? '');

  useEffect(() => {
    if (isOpen && presetLuts.length > 0) {
      setSelectedId(presetLuts[0].id);
    }
  }, [isOpen, presetLuts]);

  const handleConfirm = () => {
    if (selectedId) {
      onConfirm(selectedId);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && selectedId) {
      e.preventDefault();
      handleConfirm();
    }
  };

  return (
    <Dialog
      isOpen={isOpen}
      onClose={onCancel}
      title="LUT 滤镜"
      subtitle="使用胶片模拟滤镜处理 RAW 照片"
      icon={
        <div className="w-10 h-10 bg-gray-100 rounded-lg flex items-center justify-center">
          <Palette className="w-5 h-5 text-violet-600" />
        </div>
      }
      footer={
        <div className="flex justify-end w-full gap-2">
          <button
            onClick={onCancel}
            className="px-4 py-2 text-gray-700 bg-gray-100 rounded-lg hover:bg-gray-200 transition-colors text-sm"
          >
            取消
          </button>
          <button
            onClick={handleConfirm}
            disabled={!selectedId}
            className="px-4 py-2 text-white bg-violet-600 rounded-lg hover:bg-violet-700 transition-colors text-sm disabled:opacity-50 disabled:cursor-not-allowed"
          >
            应用
          </button>
        </div>
      }
    >
      <div className="space-y-3" onKeyDown={handleKeyDown}>
        <div>
          <label className="block text-sm font-medium text-gray-700 mb-1">滤镜</label>
          <Select
            value={selectedId}
            options={options}
            onChange={setSelectedId}
          />
        </div>
      </div>
    </Dialog>
  );
}
