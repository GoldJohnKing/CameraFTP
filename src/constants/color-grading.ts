/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import type { SelectOption } from '../types/select';

// Defaults are generated from Rust (src-tauri/src/color_grading/presets.rs)
// by export-bindings (`./build.sh gen-types`); re-exported here so existing
// consumers keep a stable import path.
export {
  DEFAULT_PRESET_ID,
  DEFAULT_METERING_MODE,
  DEFAULT_EV_OFFSET,
} from '../../src-tauri/bindings/ColorGradingDefaults';

// Metering-mode display labels are frontend-only i18n strings (see the TODO
// in presets.rs), so this option list is not generated from Rust.
export const METERING_MODES: SelectOption[] = [
  { value: 'highlight-safe', label: '高光保护' },
  { value: 'matrix', label: '矩阵测光' },
  { value: 'center-weighted', label: '中央重点测光' },
  { value: 'average', label: '平均测光' },
  { value: 'hybrid', label: '混合测光' },
];
