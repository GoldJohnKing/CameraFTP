/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { AppConfig, PreviewWindowConfig } from '../types';
import { debounce, executeAsync } from '../utils/store';

interface ConfigState {
  config: AppConfig | null;
  draft: AppConfig | null;
  isLoading: boolean;
  error: string | null;
  activeTab: 'home' | 'gallery' | 'config';
  platform: string;
  draftRevision: number;

  loadConfig: () => Promise<void>;
  updateDraft: (updater: (draft: AppConfig) => AppConfig) => void;
  commitDraft: () => Promise<void>;
  saveAuthConfig: (auth: { anonymous: boolean; username: string; password: string }) => Promise<void>;
  updatePreviewConfig: (updates: Partial<PreviewWindowConfig>) => Promise<PreviewWindowConfig | null>;
  applyPreviewConfig: (previewConfig: PreviewWindowConfig) => void;
  resetDraft: () => void;
  setAutostart: (enabled: boolean) => Promise<void>;
  setActiveTab: (tab: 'home' | 'gallery' | 'config') => void;
  loadPlatform: () => Promise<void>;
}

const DEBOUNCE_DELAY = 100;

export const useConfigStore = create<ConfigState>((set, get) => {
  let wholeConfigSavePromise: Promise<void> | null = null;
  let writeQueue: Promise<void> = Promise.resolve();

  const enqueueWrite = <T>(operation: () => Promise<T>): Promise<T> => {
    const run = async () => operation();
    const queuedOperation = writeQueue.then(run, run);
    writeQueue = queuedOperation.then(
      () => undefined,
      () => undefined,
    );
    return queuedOperation;
  };

  const mergeDraftWithBackend = (
    nextConfig: AppConfig,
    currentConfig: AppConfig | null,
    currentDraft: AppConfig | null,
    preserveMode: 'all' | 'excludeAuth',
  ): AppConfig => {
    if (!currentConfig || !currentDraft) {
      return nextConfig;
    }

    const preserveAuth = preserveMode !== 'excludeAuth'
      && currentDraft.advancedConnection.auth !== currentConfig.advancedConnection.auth;
    return {
      ...nextConfig,
      savePath: currentDraft.savePath !== currentConfig.savePath ? currentDraft.savePath : nextConfig.savePath,
      port: currentDraft.port !== currentConfig.port ? currentDraft.port : nextConfig.port,
      autoSelectPort: currentDraft.autoSelectPort !== currentConfig.autoSelectPort
        ? currentDraft.autoSelectPort
        : nextConfig.autoSelectPort,
      advancedConnection: {
        ...nextConfig.advancedConnection,
        enabled: currentDraft.advancedConnection.enabled !== currentConfig.advancedConnection.enabled
          ? currentDraft.advancedConnection.enabled
          : nextConfig.advancedConnection.enabled,
        auth: preserveAuth ? currentDraft.advancedConnection.auth : nextConfig.advancedConnection.auth,
      },
      previewConfig: nextConfig.previewConfig,
      androidImageViewer: currentDraft.androidImageViewer !== currentConfig.androidImageViewer
        ? currentDraft.androidImageViewer
        : nextConfig.androidImageViewer,
    };
  };

  const runWholeConfigSave = async (config: AppConfig, savedRevision: number) => {
    const savePromise = enqueueWrite(async () => {
      try {
        if (get().draftRevision !== savedRevision) {
          return;
        }

        await invoke('save_config', { config });
        const { draftRevision } = get();
        if (draftRevision === savedRevision) {
          set({ config, error: null });
        }
      } catch (e) {
        set({ error: String(e) });
        throw e;
      }
    });

    wholeConfigSavePromise = savePromise;
    try {
      await savePromise;
    } finally {
      if (wholeConfigSavePromise === savePromise) {
        wholeConfigSavePromise = null;
      }
    }
  };

  const debouncedSave = debounce((config: AppConfig, savedRevision: number) => {
    void runWholeConfigSave(config, savedRevision);
  }, DEBOUNCE_DELAY);

  const waitForWholeConfigSaveBarrier = async () => {
    debouncedSave.flush();
    if (wholeConfigSavePromise) {
      await wholeConfigSavePromise;
    }
  };

  const resyncFromBackend = async (preserveMode: 'all' | 'excludeAuth') => {
    const nextConfig = await invoke<AppConfig>('load_config');
    set((state) => ({
      config: nextConfig,
      draft: mergeDraftWithBackend(nextConfig, state.config, state.draft, preserveMode),
      draftRevision: state.draftRevision + 1,
      error: null,
    }));
  };

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
      await waitForWholeConfigSaveBarrier();
    },

    saveAuthConfig: async ({ anonymous, username, password }) => {
      await waitForWholeConfigSaveBarrier();
      await enqueueWrite(async () => {
        await invoke('save_auth_config', { anonymous, username, password });
        await resyncFromBackend('excludeAuth');
      });
    },

    updatePreviewConfig: async (updates) => {
      await waitForWholeConfigSaveBarrier();

      return enqueueWrite(async () => {
        const nextPreviewConfig = await invoke<PreviewWindowConfig>('update_preview_config', { patch: updates });
        await resyncFromBackend('all');
        return nextPreviewConfig;
      });
    },

    applyPreviewConfig: (previewConfig) => {
      set((state) => {
        if (!state.config || !state.draft) {
          return state;
        }

        return {
          config: {
            ...state.config,
            previewConfig,
          },
          draft: {
            ...state.draft,
            previewConfig,
          },
        };
      });
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

    setActiveTab: (tab: 'home' | 'gallery' | 'config') => {
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
