/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { create } from 'zustand';
import { useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import type { ColorGradingEvent } from '../types';
import { requestMediaLibraryRefresh } from '../utils/gallery-refresh';

interface ColorGradingProgressState {
  isProcessing: boolean;
  isDone: boolean;
  current: number;
  total: number;
  currentFileName: string;
  failedCount: number;
  failedFiles: string[];
}

const initialState: ColorGradingProgressState = {
  isProcessing: false,
  isDone: false,
  current: 0,
  total: 0,
  currentFileName: '',
  failedCount: 0,
  failedFiles: [],
};

const GALLERY_REFRESH_DELAY_MS = 500;
const DONE_AUTO_RESET_DELAY_MS = 3000;

const useColorGradingProgressStore = create<ColorGradingProgressState>(() => ({ ...initialState }));

let _listenerRegistered = false;
let _storedUnlisten: (() => void) | null = null;

function scanOutputFiles(outputFiles: string[]) {
  for (const filePath of outputFiles) {
    window.ImageViewerAndroid?.scanNewFile?.(filePath);
  }
}

function handleEvent(event: ColorGradingEvent) {
  console.debug('[color-grading-progress] Event:', event.type, event);
  switch (event.type) {
    case 'progress':
      useColorGradingProgressStore.setState({
        isProcessing: true,
        isDone: false,
        current: event.current,
        total: event.total,
        currentFileName: event.fileName,
        failedCount: event.failedCount,
      });
      break;
    case 'completed':
      useColorGradingProgressStore.setState({
        total: event.total,
        failedCount: event.failedCount,
      });
      break;
    case 'failed':
      useColorGradingProgressStore.setState({
        total: event.total,
        failedCount: event.failedCount,
      });
      break;
    case 'done': {
      const hasFailures = event.failedCount > 0;
      const outputFiles = event.outputFiles ?? [];

      if (event.cancelled) {
        useColorGradingProgressStore.setState({ ...initialState });
        scanOutputFiles(outputFiles);
        setTimeout(() => {
          requestMediaLibraryRefresh({ reason: 'color-grading' });
        }, GALLERY_REFRESH_DELAY_MS);
        break;
      }

      useColorGradingProgressStore.setState({
        isProcessing: false,
        isDone: true,
        current: event.total,
        failedCount: event.failedCount,
        failedFiles: event.failedFiles,
      });

      scanOutputFiles(outputFiles);

      setTimeout(() => {
        requestMediaLibraryRefresh({ reason: 'color-grading' });
      }, GALLERY_REFRESH_DELAY_MS);

      if (!hasFailures) {
        setTimeout(() => {
          useColorGradingProgressStore.setState({ ...initialState });
        }, DONE_AUTO_RESET_DELAY_MS);
      }
      break;
    }
  }
}

async function registerListener(): Promise<void> {
  if (_listenerRegistered) return;
  _listenerRegistered = true;

  try {
    if (_storedUnlisten) {
      _storedUnlisten();
      _storedUnlisten = null;
    }
    const unlisten = await listen<ColorGradingEvent>('color-grading-progress', (e) => {
      handleEvent(e.payload);
    });
    _storedUnlisten = unlisten;
  } catch (err) {
    _listenerRegistered = false;
    console.error('[color-grading-progress] Listener registration failed:', err);
  }
}

// Register eagerly at module load time
registerListener();

export function useColorGradingProgress(): ColorGradingProgressState {
  return useColorGradingProgressStore();
}

export async function enqueueColorGrading(files: string[], lutId: string): Promise<void> {
  await invoke('enqueue_color_grading', { filePaths: files, lutId });
}

export async function cancelColorGrading(): Promise<void> {
  await invoke('cancel_color_grading');
}

export function dismissColorGradingDone() {
  useColorGradingProgressStore.setState({ ...initialState });
}

export function useColorGradingProgressListener() {
  useEffect(() => {
    registerListener();
  }, []);
}
