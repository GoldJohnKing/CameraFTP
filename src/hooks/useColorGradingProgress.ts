/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { invoke } from '@tauri-apps/api/core';
import type { ColorGradingEvent } from '../types';
import { createTaskProgressHook } from './createTaskProgressHook';
import type { TaskProgressState } from './createTaskProgressHook';
import { DEFAULT_METERING_MODE, DEFAULT_EV_OFFSET } from '../constants/color-grading';

export interface ColorGradingProgressState {
  isProcessing: boolean;
  isDone: boolean;
  current: number;
  total: number;
  currentFileName: string;
  failedCount: number;
  failedFiles: string[];
}

function mapToState(state: TaskProgressState): ColorGradingProgressState {
  return { ...state, isProcessing: state.isActive };
}

const colorGrading = createTaskProgressHook<ColorGradingEvent>({
  eventName: 'color-grading-progress',
  debugLabel: 'color-grading',
  refreshReason: 'color-grading',
  mapEvent: (event) => {
    switch (event.type) {
      case 'progress':
        return { type: 'progress', current: event.current, total: event.total, fileName: event.fileName, failedCount: event.failedCount };
      case 'completed':
        return { type: 'completed', current: event.current, total: event.total, fileName: event.fileName, failedCount: event.failedCount, outputPath: event.outputPath };
      case 'done':
        return { type: 'done', total: event.total, failedCount: event.failedCount, failedFiles: event.failedFiles, outputFiles: event.outputFiles, cancelled: event.cancelled };
      case 'queued':
        return null;
      default:
        return null;
    }
  },
  nativeBridge: {
    syncProgress: (current, total, failedCount) => {
      window.ImageViewerAndroid?.updateColorGradingProgress?.(current, total, failedCount);
    },
    notifyDone: (success, message, cancelled) => {
      window.ImageViewerAndroid?.onColorGradingComplete?.(success, message, cancelled);
    },
  },
});

export function useColorGradingProgress(): ColorGradingProgressState {
  return mapToState(colorGrading.useProgress());
}

export async function enqueueColorGrading(
  files: string[],
  lutId: string,
  meteringMode: string = DEFAULT_METERING_MODE,
  evOffset: number = DEFAULT_EV_OFFSET,
): Promise<void> {
  await invoke('enqueue_color_grading', { filePaths: files, lutId, meteringMode, evOffset });
}

export async function cancelColorGrading(): Promise<void> {
  await invoke('cancel_color_grading');
}

export function dismissColorGradingDone() {
  colorGrading.dismissDone();
}

export function getCurrentColorGradingProgress(): ColorGradingProgressState {
  return mapToState(colorGrading.getProgressState());
}
