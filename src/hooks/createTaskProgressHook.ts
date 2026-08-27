/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useStore } from 'zustand';
import {
  createTaskProgressService,
  type TaskProgressServiceConfig,
  type TaskProgressState,
} from '../services/task-progress';

export type { TaskProgressState };

/**
 * React adapter over the framework-agnostic task-progress service
 * (`services/task-progress.ts`). The listener registration side-effect runs
 * during render (every render), matching the original hook behavior.
 */
export function createTaskProgressHook<TEvent extends { type: string }>(
  config: TaskProgressServiceConfig<TEvent>,
) {
  const { store, ensureListener, dismissDone, getProgressState } = createTaskProgressService<TEvent>(config);

  function useProgress(): TaskProgressState {
    ensureListener();
    return useStore(store);
  }

  return { useProgress, dismissDone, getProgressState };
}
