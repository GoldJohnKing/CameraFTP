/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { invoke } from '@tauri-apps/api/core';
import type { AiEditProgressEvent } from '../types';
import { createTaskProgressHook } from './createTaskProgressHook';
import type { TaskProgressState, DoneEvent } from './createTaskProgressHook';

export interface AiEditProgressState {
  isEditing: boolean;
  isDone: boolean;
  current: number;
  total: number;
  currentFileName: string;
  failedCount: number;
  failedFiles: string[];
}

function mapToState(state: TaskProgressState): AiEditProgressState {
  return { ...state, isEditing: state.isActive };
}

const aiEdit = createTaskProgressHook<AiEditProgressEvent>({
  eventName: 'ai-edit-progress',
  debugLabel: 'ai-edit',
  refreshReason: 'ai-edit',
  mapEvent: (event) => {
    switch (event.type) {
      case 'progress':
        return { type: 'progress', current: event.current, total: event.total, fileName: event.fileName, failedCount: event.failedCount };
      case 'done':
        return { type: 'done', total: event.total, failedCount: event.failedCount, failedFiles: event.failedFiles, outputFiles: event.outputFiles, cancelled: event.cancelled };
      case 'queued':
      case 'queuedDropped':
        return null;
      default:
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
    if (event.type === 'queuedDropped') {
      console.warn(
        `[ai-edit-progress] Auto-edit task dropped (queue full): ${event.fileName}`,
      );
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
  const state = aiEdit.getProgressState();
  const total = overrides?.total ?? state.total;
  const failedCount = overrides?.failedCount ?? state.failedCount;
  window.ImageViewerAndroid?.updateAiEditProgress?.(state.current, total, failedCount);
}

function notifyNativeDone(event: DoneEvent) {
  if (event.cancelled) {
    window.ImageViewerAndroid?.onAiEditComplete?.(false, null, true);
    return;
  }
  const message = event.failedCount > 0
    ? `成功${event.total - event.failedCount}张 失败${event.failedCount}张`
    : `共${event.total}张`;
  window.ImageViewerAndroid?.onAiEditComplete?.(event.failedCount === 0, message, false);
}



export function useAiEditProgress(): AiEditProgressState {
  return mapToState(aiEdit.useProgress());
}

export async function enqueueAiEdit(files: string[], prompt: string, model?: string): Promise<void> {
  await invoke('enqueue_ai_edit', {
    filePaths: files,
    prompt: prompt || null,
    model: model || null,
  });
}

export async function cancelAiEdit(): Promise<void> {
  await invoke('cancel_ai_edit');
}

export function dismissDone() {
  aiEdit.dismissDone();
}

export function getCurrentAiEditProgress(): AiEditProgressState {
  return mapToState(aiEdit.getProgressState());
}
