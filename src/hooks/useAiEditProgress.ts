/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { create } from 'zustand';
import { useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import type { AiEditProgressEvent } from '../types';
import { requestMediaLibraryRefresh } from '../utils/gallery-refresh';

interface AiEditProgressState {
  isEditing: boolean;
  isDone: boolean;
  current: number;
  total: number;
  currentFileName: string;
  failedCount: number;
  failedFiles: string[];
}

const initialState: AiEditProgressState = {
  isEditing: false,
  isDone: false,
  current: 0,
  total: 0,
  currentFileName: '',
  failedCount: 0,
  failedFiles: [],
};

const useAiEditProgressStore = create<AiEditProgressState>(() => ({ ...initialState }));

// Stored for module teardown.
let _listenerCleanup: (() => void) | null = null;
void _listenerCleanup; // Prevent TS6133: kept for module lifecycle management.
let _listenerRegistered = false;

function syncToNativeLayer(current: number, total: number, failedCount: number) {
  window.ImageViewerAndroid?.updateAiEditProgress?.(current, total, failedCount);
}

function notifyNativeDone(success: boolean, failedCount: number, failedFiles: string[]) {
  const message = success
    ? null
    : `修图完成，${failedCount}张失败：${failedFiles.join('、')}`;
  window.ImageViewerAndroid?.onAiEditComplete?.(success, message);
}

function scanOutputFiles(outputFiles: string[]) {
  for (const filePath of outputFiles) {
    window.ImageViewerAndroid?.scanNewFile?.(filePath);
  }
}

function handleEvent(event: AiEditProgressEvent) {
  console.debug('[ai-edit-progress] Event received:', event.type, event);
  switch (event.type) {
    case 'progress':
      useAiEditProgressStore.setState({
        isEditing: true,
        isDone: false,
        current: event.current,
        total: event.total,
        currentFileName: event.fileName,
        failedCount: event.failedCount,
      });
      syncToNativeLayer(event.current, event.total, event.failedCount);
      break;
    case 'completed':
      useAiEditProgressStore.setState({
        total: event.total,
        failedCount: event.failedCount,
      });
      break;
    case 'failed':
      useAiEditProgressStore.setState({
        total: event.total,
        failedCount: event.failedCount,
      });
      break;
    case 'queued': {
      const { isEditing, current, failedCount } = useAiEditProgressStore.getState();
      if (isEditing) {
        const newTotal = current + event.queueDepth;
        useAiEditProgressStore.setState({ total: newTotal });
        syncToNativeLayer(current, newTotal, failedCount);
      }
      break;
    }
    case 'done': {
      const hasFailures = event.failedCount > 0;
      const outputFiles = event.outputFiles ?? [];

      useAiEditProgressStore.setState({
        isEditing: false,
        isDone: hasFailures,
        current: event.total,
        failedCount: event.failedCount,
        failedFiles: event.failedFiles,
      });

      // Trigger Android MediaStore scan so system gallery sees the new files
      scanOutputFiles(outputFiles);

      // Delay refresh to allow MediaStore to finish indexing the scanned files.
      // Without this delay, the reload races ahead and queries MediaStore before
      // the file is indexed, causing the new image to appear missing or out of order.
      setTimeout(() => {
        requestMediaLibraryRefresh({ reason: 'ai-edit' });
      }, 500);

      // Auto-preview the first output file when auto-open is enabled on Android
      if (outputFiles.length > 0 && !hasFailures) {
        void autoPreviewIfEnabled(outputFiles);
      }

      if (!hasFailures) {
        setTimeout(() => {
          useAiEditProgressStore.setState({ ...initialState });
        }, 500);
      }
      notifyNativeDone(event.failedCount === 0, event.failedCount, event.failedFiles);
      break;
    }
  }
}

async function autoPreviewIfEnabled(outputFiles: string[]) {
  try {
    const { useConfigStore: _useConfigStore } = await import('../stores/configStore');
    const autoOpen = _useConfigStore.getState().draft?.androidImageViewer?.autoOpenLatestWhenVisible ?? false;
    if (autoOpen) {
      void autoPreviewOutput(outputFiles);
    }
  } catch {
    // Non-critical: auto-preview is a convenience feature
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

async function registerListener(): Promise<void> {
  if (_listenerRegistered) return;
  _listenerRegistered = true;

  try {
    const unlisten = await listen<AiEditProgressEvent>('ai-edit-progress', (e) => {
      handleEvent(e.payload);
    });
    _listenerCleanup = unlisten;
  } catch (err) {
    _listenerRegistered = false;
    console.error('[ai-edit-progress] Listener registration failed:', err);
  }
}

// Register eagerly at module load time
registerListener();

export function useAiEditProgress(): AiEditProgressState {
  return useAiEditProgressStore();
}

export async function enqueueAiEdit(files: string[], prompt: string, _shouldSave: boolean): Promise<void> {
  await invoke('enqueue_ai_edit', {
    filePaths: files,
    prompt: prompt || null,
  });
}

export async function cancelAiEdit(): Promise<void> {
  await invoke('cancel_ai_edit');
}

export function dismissDone() {
  useAiEditProgressStore.setState({ ...initialState });
}

export function getCurrentAiEditProgress(): AiEditProgressState {
  return useAiEditProgressStore.getState();
}

export function useAiEditProgressListener() {
  // Fallback: ensure listener is registered even if module-load registration failed.
  useEffect(() => {
    registerListener();
  }, []);
}
