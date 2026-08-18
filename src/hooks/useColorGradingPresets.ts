/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { ColorGradingPreset } from '../types';

let _cachedPresets: ColorGradingPreset[] = [];
let _inflightPromise: Promise<ColorGradingPreset[]> | null = null;

export function getCachedColorGradingPresets(): ColorGradingPreset[] {
  return _cachedPresets;
}

/**
 * Fetch presets, deduplicating concurrent callers on a single shared promise
 * (multiple components mount and fetch at the same time). On failure the
 * shared promise is cleared so a later mount can retry.
 */
function loadPresets(): Promise<ColorGradingPreset[]> {
  if (!_inflightPromise) {
    _inflightPromise = invoke<ColorGradingPreset[]>('get_color_grading_presets')
      .then((result) => {
        _cachedPresets = result;
        return result;
      })
      .catch((e) => {
        console.error('Failed to load color grading presets:', e);
        _inflightPromise = null;
        return _cachedPresets;
      });
  }
  return _inflightPromise;
}

export function useColorGradingPresets() {
  const [presets, setPresets] = useState<ColorGradingPreset[]>(_cachedPresets);

  useEffect(() => {
    let cancelled = false;
    void loadPresets().then((result) => {
      if (!cancelled) {
        setPresets(result);
      }
    });
    return () => {
      cancelled = true;
    };
  }, []);

  return presets;
}
