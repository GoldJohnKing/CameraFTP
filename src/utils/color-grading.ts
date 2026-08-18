/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import type { AppConfig, ColorGradingLastUsed } from '../types';

/**
 * Build the next config draft after recording the last-used color grading
 * parameters. Pure function — returns a new draft without mutating the input.
 *
 * When `syncToAuto` is true and an `autoColorGrading` config exists, the same
 * preset/metering/EV values are mirrored into it (preserving `enabled`).
 */
export function applyColorGradingLastUsed(
  draft: AppConfig,
  lastUsed: ColorGradingLastUsed,
  syncToAuto: boolean,
): AppConfig {
  const { presetId, meteringMode, evOffset } = lastUsed;
  return {
    ...draft,
    colorGradingLastUsed: {
      presetId,
      meteringMode,
      evOffset,
    },
    ...(syncToAuto && draft.autoColorGrading ? {
      autoColorGrading: {
        ...draft.autoColorGrading,
        presetId,
        meteringMode,
        evOffset,
      },
    } : {}),
  };
}
