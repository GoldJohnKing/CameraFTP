/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useStore } from 'zustand';
import {
  aiEditStore,
  ensureAiEditListener,
  mapToState,
  type AiEditProgressState,
} from '../services/ai-edit-tasks';

export function useAiEditProgress(): AiEditProgressState {
  ensureAiEditListener();
  return mapToState(useStore(aiEditStore));
}
