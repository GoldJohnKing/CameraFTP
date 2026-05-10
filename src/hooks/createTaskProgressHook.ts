/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { create, type StoreApi } from 'zustand';
import { useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { requestMediaLibraryRefresh } from '../utils/gallery-refresh';

export interface TaskProgressState {
  isActive: boolean;
  isDone: boolean;
  current: number;
  total: number;
  currentFileName: string;
  failedCount: number;
  failedFiles: string[];
}

export const initialTaskProgressState: TaskProgressState = {
  isActive: false,
  isDone: false,
  current: 0,
  total: 0,
  currentFileName: '',
  failedCount: 0,
  failedFiles: [],
};

const GALLERY_REFRESH_DELAY_MS = 500;
const DONE_AUTO_RESET_DELAY_MS = 3000;

/** Discriminated union for type-safe event switching inside the factory. */
export type StandardTaskEvent =
  | { type: 'progress'; current: number; total: number; fileName: string; failedCount: number }
  | { type: 'completed'; total: number; failedCount: number }
  | { type: 'failed'; total: number; failedCount: number }
  | { type: 'done'; total: number; failedCount: number; failedFiles: string[]; outputFiles: string[]; cancelled: boolean };

/** Shape of the `done` event — used in `onDone` callback. */
export type DoneEvent = Extract<StandardTaskEvent, { type: 'done' }>;

export interface TaskProgressHookConfig<TEvent extends { type: string }> {
  eventName: string;
  debugLabel: string;
  /** Map a domain event to a standard event. Return null to skip. */
  mapEvent: (event: TEvent) => StandardTaskEvent | null;
  /** Handle raw events that don't map to standard ones (e.g. 'queued'). Called BEFORE store update. */
  onRawEvent?: (event: TEvent, store: StoreApi<TaskProgressState>) => void;
  /** Called after the factory processes a 'done' event. */
  onDone?: (event: DoneEvent) => void;
  /** Called after the store is updated for a mapped event. */
  onAfterUpdate?: (mapped: StandardTaskEvent, store: StoreApi<TaskProgressState>) => void;
}

export function createTaskProgressHook<TEvent extends { type: string }>(
  config: TaskProgressHookConfig<TEvent>,
) {
  const store = create<TaskProgressState>(() => ({ ...initialTaskProgressState }));

  let listenerRegistered = false;
  let storedUnlisten: (() => void) | null = null;

  function scanOutputFiles(outputFiles: string[]) {
    for (const filePath of outputFiles) {
      window.ImageViewerAndroid?.scanNewFile?.(filePath);
    }
  }

  function handleEvent(event: TEvent) {
    config.onRawEvent?.(event, store);

    const mapped = config.mapEvent(event);
    if (!mapped) return;

    switch (mapped.type) {
      case 'progress':
        store.setState({
          isActive: true,
          isDone: false,
          current: mapped.current,
          total: mapped.total,
          currentFileName: mapped.fileName,
          failedCount: mapped.failedCount,
        });
        break;
      case 'completed':
        store.setState({
          total: mapped.total,
          failedCount: mapped.failedCount,
        });
        break;
      case 'failed':
        store.setState({
          total: mapped.total,
          failedCount: mapped.failedCount,
        });
        break;
      case 'done': {
        const hasFailures = mapped.failedCount > 0;
        const outputFiles = mapped.outputFiles ?? [];

        if (mapped.cancelled) {
          store.setState({ ...initialTaskProgressState });
          config.onAfterUpdate?.(mapped, store);
          scanOutputFiles(outputFiles);
          setTimeout(() => {
            requestMediaLibraryRefresh({ reason: config.debugLabel as any });
          }, GALLERY_REFRESH_DELAY_MS);
          config.onDone?.(mapped);
          break;
        }

        store.setState({
          isActive: false,
          isDone: true,
          current: mapped.total,
          failedCount: mapped.failedCount,
          failedFiles: mapped.failedFiles,
        });

        config.onAfterUpdate?.(mapped, store);

        scanOutputFiles(outputFiles);

        setTimeout(() => {
          requestMediaLibraryRefresh({ reason: config.debugLabel as any });
        }, GALLERY_REFRESH_DELAY_MS);

        config.onDone?.(mapped);

        if (!hasFailures) {
          setTimeout(() => {
            store.setState({ ...initialTaskProgressState });
          }, DONE_AUTO_RESET_DELAY_MS);
        }
        break;
      }
    }

    if (mapped.type !== 'done') {
      config.onAfterUpdate?.(mapped, store);
    }
  }

  async function registerListener(): Promise<void> {
    if (listenerRegistered) return;
    listenerRegistered = true;

    try {
      if (storedUnlisten) {
        storedUnlisten();
        storedUnlisten = null;
      }
      const unlisten = await listen<TEvent>(config.eventName, (e) => {
        handleEvent(e.payload);
      });
      storedUnlisten = unlisten;
    } catch (err) {
      listenerRegistered = false;
      console.error(`[${config.debugLabel}] Listener registration failed:`, err);
    }
  }

  // Register eagerly at module load time
  registerListener();

  function useProgress(): TaskProgressState {
    return store();
  }

  function dismissDone() {
    store.setState({ ...initialTaskProgressState });
  }

  function useProgressListener() {
    useEffect(() => {
      registerListener();
    }, []);
  }

  function getProgressState(): TaskProgressState {
    return store.getState();
  }

  return { useProgress, dismissDone, useProgressListener, getProgressState };
}
