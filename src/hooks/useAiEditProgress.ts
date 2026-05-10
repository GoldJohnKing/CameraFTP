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
      case 'queuedDropped':
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
  onDone: (event) => {
    syncToNativeLayer(event);
    notifyNativeDone(event);
    if (!event.cancelled && event.outputFiles.length > 0 && event.failedCount === 0) {
      void autoPreviewIfEnabled(event.outputFiles);
    }
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

async function autoPreviewIfEnabled(outputFiles: string[]) {
  try {
    const { useConfigStore: _useConfigStore } = await import('../stores/configStore');
    const autoOpen = _useConfigStore.getState().draft?.androidImageViewer?.autoOpenLatestWhenVisible ?? false;
    if (autoOpen) {
      void autoPreviewOutput(outputFiles);
    }
  } catch {
    // Non-critical
  }
}

async function autoPreviewOutput(outputFiles: string[]) {
  if (outputFiles.length === 0) return;

  const { openImagePreview } = await import('../services/image-open');
  const { useConfigStore: _useConfigStore } = await import('../stores/configStore');
  const openMethod = _useConfigStore.getState().draft?.androidImageViewer?.openMethod;

  const firstFile = outputFiles[0];
  const allUris = [firstFile];

  void openImagePreview({
    filePath: firstFile,
    openMethod,
    allUris,
  });
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

export function useAiEditProgressListener() {
  aiEdit.useProgressListener();
}
