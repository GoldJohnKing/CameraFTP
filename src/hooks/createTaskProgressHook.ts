/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { create, type StoreApi } from 'zustand';
import { listen } from '@tauri-apps/api/event';
import { requestMediaLibraryRefresh, type MediaLibraryRefreshReason } from '../utils/gallery-refresh';

export interface TaskProgressState {
  isActive: boolean;
  isDone: boolean;
  current: number;
  total: number;
  currentFileName: string;
  failedCount: number;
  failedFiles: string[];
}

const initialTaskProgressState: TaskProgressState = {
  isActive: false,
  isDone: false,
  current: 0,
  total: 0,
  currentFileName: '',
  failedCount: 0,
  failedFiles: [],
};

const GALLERY_REFRESH_DELAY_MS = 500;

/** Discriminated union for type-safe event switching inside the factory. */
export type StandardTaskEvent =
  | { type: 'progress'; current: number; total: number; fileName: string; failedCount: number }
  | { type: 'completed'; current: number; total: number; fileName: string; failedCount: number; outputPath?: string }
  | { type: 'done'; total: number; failedCount: number; failedFiles: string[]; outputFiles: string[]; cancelled: boolean };

/** Shape of the `done` event — used in `onDone` callback. */
export type DoneEvent = Extract<StandardTaskEvent, { type: 'done' }>;

/**
 * Native-layer adapter. Provide one to let the factory handle the common
 * progress/done glue (native progress sync, queued-total expansion, and the
 * done notification) instead of duplicating it per hook.
 */
export interface TaskProgressNativeBridge {
  /** Push (current, total, failedCount) to the native progress UI. */
  syncProgress: (current: number, total: number, failedCount: number) => void;
  /** Notify the native layer that the batch finished. */
  notifyDone: (success: boolean, message: string | null, cancelled: boolean) => void;
}

export interface TaskProgressHookConfig<TEvent extends { type: string }> {
  eventName: string;
  debugLabel: string;
  refreshReason: MediaLibraryRefreshReason;
  /** Map a domain event to a standard event. Return null to skip. */
  mapEvent: (event: TEvent) => StandardTaskEvent | null;
  /** Handle raw events that don't map to standard ones (e.g. 'queued'). Called BEFORE store update. */
  onRawEvent?: (event: TEvent, store: StoreApi<TaskProgressState>) => void;
  /** Called after the factory processes a 'done' event. */
  onDone?: (event: DoneEvent) => void;
  /** Called after the store is updated for a mapped event. */
  onAfterUpdate?: (mapped: StandardTaskEvent, store: StoreApi<TaskProgressState>) => void;
  /** Optional native-layer adapter; enables the shared progress/done glue. */
  nativeBridge?: TaskProgressNativeBridge;
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

  /** Push the current (or overridden) progress to the native layer. */
  function syncNativeProgress(overrides?: { total?: number; failedCount?: number }) {
    if (!config.nativeBridge) return;
    const state = store.getState();
    const total = overrides?.total ?? state.total;
    const failedCount = overrides?.failedCount ?? state.failedCount;
    config.nativeBridge.syncProgress(state.current, total, failedCount);
  }

  /**
   * Expand `total` when new items are queued while a batch is active, and
   * sync the expanded total to the native layer.
   */
  function handleQueued(event: TEvent) {
    const queued = event as { type: string; queueDepth?: unknown };
    if (queued.type !== 'queued' || typeof queued.queueDepth !== 'number') return;
    const state = store.getState();
    if (!state.isActive) return;
    const newTotal = state.current + queued.queueDepth;
    store.setState({ total: newTotal });
    syncNativeProgress({ total: newTotal, failedCount: state.failedCount });
  }

  /** Sync final progress and notify the native layer that the batch ended. */
  function finishNativeDone(event: DoneEvent) {
    if (!config.nativeBridge) return;
    syncNativeProgress(event);
    if (event.cancelled) {
      config.nativeBridge.notifyDone(false, null, true);
      return;
    }
    const message = event.failedCount > 0
      ? `成功${event.total - event.failedCount}张 失败${event.failedCount}张`
      : `共${event.total}张`;
    config.nativeBridge.notifyDone(event.failedCount === 0, message, false);
  }

  function handleEvent(event: TEvent) {
    config.onRawEvent?.(event, store);
    handleQueued(event);

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
        config.onAfterUpdate?.(mapped, store);
        syncNativeProgress();
        break;
      case 'completed': {
        // Per-file completion: scan immediately and refresh after a short delay
        // so the gallery updates as each image is produced, not only when the batch ends.
        const completedOutputPath = mapped.outputPath;
        if (completedOutputPath) {
          scanOutputFiles([completedOutputPath]);
          setTimeout(() => {
            requestMediaLibraryRefresh({ reason: config.refreshReason });
          }, GALLERY_REFRESH_DELAY_MS);
        }
        store.setState({
          current: mapped.current,
          total: mapped.total,
          currentFileName: mapped.fileName,
          failedCount: mapped.failedCount,
        });
        config.onAfterUpdate?.(mapped, store);
        break;
      }
      case 'done': {
        const outputFiles = mapped.outputFiles ?? [];

        if (mapped.cancelled) {
          store.setState({ ...initialTaskProgressState });
          config.onAfterUpdate?.(mapped, store);
          scanOutputFiles(outputFiles);
          setTimeout(() => {
            requestMediaLibraryRefresh({ reason: config.refreshReason });
          }, GALLERY_REFRESH_DELAY_MS);
          finishNativeDone(mapped);
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

        // Final batch-level scan for any files not yet scanned (edge cases),
        // plus a concluding refresh to ensure the gallery is fully up-to-date.
        scanOutputFiles(outputFiles);
        setTimeout(() => {
          requestMediaLibraryRefresh({ reason: config.refreshReason });
        }, GALLERY_REFRESH_DELAY_MS);

        finishNativeDone(mapped);
        config.onDone?.(mapped);
        break;
      }
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

  function ensureListener() {
    if (!listenerRegistered) {
      void registerListener();
    }
  }

  function useProgress(): TaskProgressState {
    ensureListener();
    return store();
  }

  function dismissDone() {
    store.setState({ ...initialTaskProgressState });
  }

  function getProgressState(): TaskProgressState {
    return store.getState();
  }

  return { useProgress, dismissDone, getProgressState };
}
