/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { toast } from 'sonner';
import type { AppConfig, PreviewWindowConfig } from '../types';
import { formatError } from '../utils/error';
import { debounce, executeAsync } from '../utils/store';

interface ConfigState {
  config: AppConfig | null;
  draft: AppConfig | null;
  isLoading: boolean;
  error: string | null;
  activeTab: 'home' | 'gallery' | 'config';
  platform: string;

  loadConfig: () => Promise<void>;
  updateDraft: (updater: (draft: AppConfig) => AppConfig) => void;
  flushConfigSave: () => Promise<void>;
  saveAuthConfig: (auth: { anonymous: boolean; username: string; password: string }) => Promise<void>;
  updatePreviewConfig: (updates: Partial<PreviewWindowConfig>) => Promise<PreviewWindowConfig | null>;
  applyPreviewConfig: (previewConfig: PreviewWindowConfig) => void;
  setAutostart: (enabled: boolean) => Promise<void>;
  setActiveTab: (tab: 'home' | 'gallery' | 'config') => void;
  loadPlatform: () => Promise<void>;
}

const DEBOUNCE_DELAY = 100;

// Business keys of the config draft that survive a backend resync while dirty
// (reference-unequal to the last-known saved config). Must stay in lockstep
// with the generated AppConfig binding: a key missing from this list would be
// silently clobbered by mergeDraftWithBackend on the next resync, and a key
// that no longer exists on AppConfig is a compile error via `satisfies`.
// Exhaustiveness is enforced at runtime by the keys-coverage test in
// __tests__/configStore.test.ts.
export const DRAFT_PRESERVED_KEYS = [
  'savePath', 'port', 'autoSelectPort', 'advancedConnection', 'previewConfig',
  'androidImageViewer', 'aiEdit', 'autoColorGrading', 'colorGradingLastUsed',
] as const satisfies readonly (keyof AppConfig)[];

let draftRevision = 0;

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

  const preserveIfDirty = <K extends keyof AppConfig>(
    next: AppConfig, current: AppConfig, draft: AppConfig, key: K,
  ): AppConfig[K] =>
    draft[key] !== current[key] ? draft[key] : next[key];

  const mergeDraftWithBackend = (
    nextConfig: AppConfig,
    currentConfig: AppConfig | null,
    currentDraft: AppConfig | null,
    preserveMode: 'all' | 'excludeAuth',
  ): AppConfig => {
    if (!currentConfig || !currentDraft) {
      return nextConfig;
    }

    // advancedConnection merges per sub-field: a draft edit that only touched
    // `auth` must not also pin a stale `enabled` (and vice versa).
    const preserveAdvancedEnabled = currentDraft.advancedConnection.enabled
      !== currentConfig.advancedConnection.enabled;
    const preserveAuth = preserveMode !== 'excludeAuth'
      && currentDraft.advancedConnection.auth !== currentConfig.advancedConnection.auth;

    const merged: AppConfig = {
      ...nextConfig,
      advancedConnection: {
        ...nextConfig.advancedConnection,
        enabled: preserveAdvancedEnabled
          ? currentDraft.advancedConnection.enabled
          : nextConfig.advancedConnection.enabled,
        auth: preserveAuth ? currentDraft.advancedConnection.auth : nextConfig.advancedConnection.auth,
      },
    };

    // 泛型辅助：通过 T 延迟联合键的类型解析，避免联合键直写 merged[key]
    // 被收窄为 never（TS 对联合键写入要求可赋给各字段的交集）。
    const applyPreserved = <T extends keyof AppConfig>(
      merged: AppConfig,
      nextConfig: AppConfig,
      currentConfig: AppConfig,
      currentDraft: AppConfig,
      keys: readonly T[],
    ): void => {
      for (const key of keys) {
        merged[key] = preserveIfDirty(nextConfig, currentConfig, currentDraft, key);
      }
    };

    // advancedConnection 与 previewConfig 不走 preserve-if-dirty：
    // 前者上方已按子字段合并，后者为后端所有（仅经 update_preview_config 写入）。
    applyPreserved(
      merged,
      nextConfig,
      currentConfig,
      currentDraft,
      DRAFT_PRESERVED_KEYS.filter(
        (key): key is Exclude<(typeof DRAFT_PRESERVED_KEYS)[number], 'advancedConnection' | 'previewConfig'> =>
          key !== 'advancedConnection' && key !== 'previewConfig',
      ),
    );

    return merged;
  };

  const runWholeConfigSave = async (config: AppConfig, savedRevision: number) => {
    const savePromise = enqueueWrite(async () => {
      try {
        if (draftRevision !== savedRevision) {
          return;
        }

        await invoke('save_config', { config });
        if (draftRevision === savedRevision) {
          set({ config, error: null });
        }
      } catch (e) {
        console.error('Failed to save config:', e);
        const errorMessage = '配置保存失败：' + formatError(e);
        toast.error(errorMessage);
        set({ error: errorMessage });
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
    draftRevision += 1;
    set((state) => ({
      config: nextConfig,
      draft: mergeDraftWithBackend(nextConfig, state.config, state.draft, preserveMode),
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

    loadConfig: async () => {
      await executeAsync(
        {
          operation: () => invoke<AppConfig>('load_config'),
          onSuccess: (config, set) => {
            set((state) => ({ ...state, config, draft: config }));
          },
        },
        set,
      );
    },

    updateDraft: (updater: (draft: AppConfig) => AppConfig) => {
      const { draft } = get();
      if (!draft) return;

      const newDraft = updater(draft);
      draftRevision += 1;
      set({ draft: newDraft });

      debouncedSave(newDraft, draftRevision);
    },

    flushConfigSave: async () => {
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

    // Note: This doesn't modify global isLoading to avoid triggering re-renders
    setAutostart: async (enabled: boolean) => {
      await invoke('set_autostart_command', { enable: enabled });
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
