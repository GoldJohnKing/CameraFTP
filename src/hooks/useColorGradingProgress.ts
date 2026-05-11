/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { invoke } from '@tauri-apps/api/core';
import type { ColorGradingEvent } from '../types';
import { createTaskProgressHook } from './createTaskProgressHook';
import type { TaskProgressState, DoneEvent } from './createTaskProgressHook';

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
  mapEvent: (event) => {
    switch (event.type) {
      case 'progress':
        return { type: 'progress', current: event.current, total: event.total, fileName: event.fileName, failedCount: event.failedCount };
      case 'completed':
        return { type: 'completed', total: event.total, failedCount: event.failedCount };
      case 'failed':
        return { type: 'failed', total: event.total, failedCount: event.failedCount };
      case 'done':
        return { type: 'done', total: event.total, failedCount: event.failedCount, failedFiles: event.failedFiles, outputFiles: event.outputFiles, cancelled: event.cancelled };
      case 'queued':
        return null;
    }
  },
  onRawEvent: (event, store) => {
    if (event.type === 'queued') {
      const state = store.getState();
      if (state.isActive) {
        const newTotal = state.current + event.queueDepth;
        store.setState({ total: newTotal });
        syncToNativeLayer({ total: newTotal, failedCount: state.failedCount });
      }
    }
  },
  onAfterUpdate: (mapped) => {
    if (mapped.type === 'progress') {
      syncToNativeLayer();
    }
  },
  onDone: (event) => {
    syncToNativeLayer(event);
    notifyNativeDone(event);
  },
});

function syncToNativeLayer(overrides?: { total?: number; failedCount?: number }) {
  const state = colorGrading.getProgressState();
  const total = overrides?.total ?? state.total;
  const failedCount = overrides?.failedCount ?? state.failedCount;
  window.ImageViewerAndroid?.updateColorGradingProgress?.(state.current, total, failedCount);
}

function notifyNativeDone(event: DoneEvent) {
  if (event.cancelled) {
    window.ImageViewerAndroid?.onColorGradingComplete?.(false, null, true);
    return;
  }
  const message = event.failedCount > 0
    ? `成功${event.total - event.failedCount}张 失败${event.failedCount}张`
    : `共${event.total}张`;
  window.ImageViewerAndroid?.onColorGradingComplete?.(event.failedCount === 0, message, false);
}

export function useColorGradingProgress(): ColorGradingProgressState {
  return mapToState(colorGrading.useProgress());
}

export async function enqueueColorGrading(
  files: string[],
  lutId: string,
  useAutoExposure: boolean = true,
  meteringMode: string = 'highlight-safe',
  manualEv: number = 0.0,
): Promise<void> {
  await invoke('enqueue_color_grading', { filePaths: files, lutId, useAutoExposure, meteringMode, manualEv });
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

export function useColorGradingProgressListener() {
  colorGrading.useProgressListener();
}
