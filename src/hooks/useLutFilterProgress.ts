/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { create } from 'zustand';
import { useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import type { LutFilterProgressEvent } from '../types';
import { requestMediaLibraryRefresh } from '../utils/gallery-refresh';

interface LutFilterProgressState {
  isProcessing: boolean;
  isDone: boolean;
  current: number;
  total: number;
  currentFileName: string;
  failedCount: number;
  failedFiles: string[];
}

const initialState: LutFilterProgressState = {
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

const useLutFilterProgressStore = create<LutFilterProgressState>(() => ({ ...initialState }));

let _listenerRegistered = false;
let _storedUnlisten: (() => void) | null = null;

function scanOutputFiles(outputFiles: string[]) {
  for (const filePath of outputFiles) {
    window.ImageViewerAndroid?.scanNewFile?.(filePath);
  }
}

function handleEvent(event: LutFilterProgressEvent) {
  console.debug('[lut-filter-progress] Event:', event.type, event);
  switch (event.type) {
    case 'progress':
      useLutFilterProgressStore.setState({
        isProcessing: true,
        isDone: false,
        current: event.current,
        total: event.total,
        currentFileName: event.fileName,
        failedCount: event.failedCount,
      });
      break;
    case 'completed':
      useLutFilterProgressStore.setState({
        total: event.total,
        failedCount: event.failedCount,
      });
      break;
    case 'failed':
      useLutFilterProgressStore.setState({
        total: event.total,
        failedCount: event.failedCount,
      });
      break;
    case 'done': {
      const hasFailures = event.failedCount > 0;
      const outputFiles = event.outputFiles ?? [];

      if (event.cancelled) {
        useLutFilterProgressStore.setState({ ...initialState });
        scanOutputFiles(outputFiles);
        setTimeout(() => {
          requestMediaLibraryRefresh({ reason: 'lut-filter' });
        }, GALLERY_REFRESH_DELAY_MS);
        break;
      }

      useLutFilterProgressStore.setState({
        isProcessing: false,
        isDone: true,
        current: event.total,
        failedCount: event.failedCount,
        failedFiles: event.failedFiles,
      });

      scanOutputFiles(outputFiles);

      setTimeout(() => {
        requestMediaLibraryRefresh({ reason: 'lut-filter' });
      }, GALLERY_REFRESH_DELAY_MS);

      if (!hasFailures) {
        setTimeout(() => {
          useLutFilterProgressStore.setState({ ...initialState });
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
    const unlisten = await listen<LutFilterProgressEvent>('lut-filter-progress', (e) => {
      handleEvent(e.payload);
    });
    _storedUnlisten = unlisten;
  } catch (err) {
    _listenerRegistered = false;
    console.error('[lut-filter-progress] Listener registration failed:', err);
  }
}

// Register eagerly at module load time
registerListener();

export function useLutFilterProgress(): LutFilterProgressState {
  return useLutFilterProgressStore();
}

export async function enqueueLutFilter(files: string[], lutId: string): Promise<void> {
  await invoke('enqueue_lut_filter', { filePaths: files, lutId });
}

export async function cancelLutFilter(): Promise<void> {
  await invoke('cancel_lut_filter');
}

export function dismissLutFilterDone() {
  useLutFilterProgressStore.setState({ ...initialState });
}

export function useLutFilterProgressListener() {
  useEffect(() => {
    registerListener();
  }, []);
}
