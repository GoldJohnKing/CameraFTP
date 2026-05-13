/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { ColorGradingPreset } from '../types';

let _cachedPresets: ColorGradingPreset[] = [];

export function getCachedColorGradingPresets(): ColorGradingPreset[] {
  return _cachedPresets;
}

export function useColorGradingPresets() {
  const [presets, setPresets] = useState<ColorGradingPreset[]>(_cachedPresets);

  useEffect(() => {
    invoke<ColorGradingPreset[]>('get_color_grading_presets')
      .then(result => {
        _cachedPresets = result;
        setPresets(result);
      })
      .catch((e) => console.error('Failed to load color grading presets:', e));
  }, []);

  return presets;
}
