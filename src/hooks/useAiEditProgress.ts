/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { invoke } from '@tauri-apps/api/core';
import type { AiEditProgressEvent } from '../types';
import { createTaskProgressHook } from './createTaskProgressHook';
import type { TaskProgressState } from './createTaskProgressHook';
import { useConfigStore } from '../stores/configStore';

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
      case 'completed':
        return { type: 'completed', current: event.current, total: event.total, fileName: event.fileName, failedCount: event.failedCount, outputPath: event.outputPath ?? undefined };
      case 'done':
        return { type: 'done', total: event.total, failedCount: event.failedCount, failedFiles: event.failedFiles, outputFiles: event.outputFiles, cancelled: event.cancelled };
      case 'queued':
      case 'queuedDropped':
        return null;
      default:
        return null;
    }
  },
  onRawEvent: (event) => {
    if (event.type === 'queuedDropped') {
      console.warn(
        `[ai-edit-progress] Auto-edit task dropped (queue full): ${event.fileName}`,
      );
    }
  },
  nativeBridge: {
    syncProgress: (current, total, failedCount) => {
      window.ImageViewerAndroid?.updateAiEditProgress?.(current, total, failedCount);
    },
    notifyDone: (success, message, cancelled) => {
      window.ImageViewerAndroid?.onAiEditComplete?.(success, message, cancelled);
    },
  },
});



export function useAiEditProgress(): AiEditProgressState {
  return mapToState(aiEdit.useProgress());
}

async function enqueueAiEdit(files: string[], prompt: string, model?: string): Promise<void> {
  await invoke('enqueue_ai_edit', {
    filePaths: files,
    prompt: prompt || null,
    model: model || null,
  });
}

/**
 * Save AI edit parameters to config and enqueue the edit task.
 * Shared by all AI edit trigger paths (PreviewWindow, GalleryCard, Android bridge).
 */
export async function applyAndEnqueueAiEdit(opts: {
  filePaths: string[];
  prompt: string;
  model: string;
  saveAsAutoEdit?: boolean;
  apiKey?: string;
}): Promise<void> {
  const { filePaths, prompt, model, saveAsAutoEdit, apiKey } = opts;
  const { updateDraft, flushConfigSave } = useConfigStore.getState();

  updateDraft(d => ({
    ...d,
    aiEdit: {
      ...d.aiEdit,
      manualPrompt: prompt,
      manualModel: model,
      ...(apiKey ? { provider: { ...d.aiEdit.provider, apiKey } } : {}),
      ...(saveAsAutoEdit ? {
        prompt,
        provider: {
          ...d.aiEdit.provider,
          model: model || d.aiEdit.provider.model,
          ...(apiKey ? { apiKey } : {}),
        },
      } : {}),
    },
  }));

  if (apiKey) {
    await flushConfigSave();
  }

  await enqueueAiEdit(filePaths, prompt, model);
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
