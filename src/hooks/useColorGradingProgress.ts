/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useStore } from 'zustand';
import {
  colorGradingStore,
  ensureColorGradingListener,
  mapToState,
  type ColorGradingProgressState,
} from '../services/color-grading-tasks';

export function useColorGradingProgress(): ColorGradingProgressState {
  ensureColorGradingListener();
  return mapToState(useStore(colorGradingStore));
}
