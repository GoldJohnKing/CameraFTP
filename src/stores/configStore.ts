/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { AppConfig } from '../types';
import { executeAsync } from '../utils/store';

function debounce<T extends (...args: any[]) => any>(
  fn: T,
  delay: number
): T & { cancel: () => void; flush: () => void } {
  let timeoutId: ReturnType<typeof setTimeout> | null = null;
  let lastArgs: Parameters<T> | null = null;

  const debounced = (...args: Parameters<T>) => {
    lastArgs = args;
    if (timeoutId) clearTimeout(timeoutId);
    timeoutId = setTimeout(() => {
      if (lastArgs) fn(...lastArgs);
      timeoutId = null;
      lastArgs = null;
    }, delay);
  };

  debounced.cancel = () => {
    if (timeoutId) clearTimeout(timeoutId);
    timeoutId = null;
    lastArgs = null;
  };

  debounced.flush = () => {
    if (timeoutId && lastArgs) {
      clearTimeout(timeoutId);
      fn(...lastArgs);
      timeoutId = null;
      lastArgs = null;
    }
  };

  return debounced as T & { cancel: () => void; flush: () => void };
}

interface ConfigState {
  config: AppConfig | null;
  draft: AppConfig | null;
  isLoading: boolean;
  error: string | null;
  activeTab: 'home' | 'config';
  platform: string;
  draftRevision: number;

  loadConfig: () => Promise<void>;
  updateDraft: (updater: (draft: AppConfig) => AppConfig) => void;
  commitDraft: () => Promise<void>;
  resetDraft: () => void;
  setAutostart: (enabled: boolean) => Promise<void>;
  setActiveTab: (tab: 'home' | 'config') => void;
  loadPlatform: () => Promise<void>;
}

const DEBOUNCE_DELAY = 100;

export const useConfigStore = create<ConfigState>((set, get) => {
  const debouncedSave = debounce(async (config: AppConfig, savedRevision: number) => {
    try {
      await invoke('save_config', { config });
      // Only update persisted config if draft hasn't changed since save started
      const { draftRevision } = get();
      if (draftRevision === savedRevision) {
        set({ config, error: null });
      }
    } catch (e) {
      set({ error: String(e) });
      throw e;
    }
  }, DEBOUNCE_DELAY);

  return {
    config: null,
    draft: null,
    isLoading: false,
    error: null,
    activeTab: 'home',
    platform: 'unknown',
    draftRevision: 0,

    loadConfig: async () => {
      await executeAsync(
        {
          operation: () => invoke<AppConfig>('load_config'),
          onSuccess: (config, set) => set((state) => ({ ...state, config, draft: config })),
        },
        set,
      );
    },

    updateDraft: (updater: (draft: AppConfig) => AppConfig) => {
      const { draft, draftRevision } = get();
      if (!draft) return;

      const newDraft = updater(draft);
      const newRevision = draftRevision + 1;
      set({ draft: newDraft, draftRevision: newRevision });

      debouncedSave(newDraft, newRevision);
    },

    commitDraft: async () => {
      debouncedSave.flush();
    },

    resetDraft: () => {
      const { config } = get();
      if (config) {
        set({ draft: config });
        debouncedSave.cancel();
      }
    },

    // Note: This doesn't modify global isLoading to avoid triggering re-renders
    setAutostart: async (enabled: boolean) => {
      try {
        await invoke('set_autostart_command', { enable: enabled });
      } catch (e) {
        throw e;
      }
    },

    setActiveTab: (tab: 'home' | 'config') => {
      set({ activeTab: tab });
    },

    loadPlatform: (() => {
      let didLoad = false;
      return async () => {
        if (didLoad) return;
        didLoad = true;

        try {
          const platformValue = await invoke<string>('get_platform');
          set({ platform: platformValue });
        } catch {
          set({ platform: 'unknown' });
        }
      };
    })(),
  };
});

export const useDraftConfig = () => useConfigStore(state => state.draft);

export const useSavedConfig = () => useConfigStore(state => state.config);
