import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { AppConfig } from '../types';
import { executeAsync } from '../utils/store';

interface ConfigState {
  config: AppConfig | null;
  isLoading: boolean;
  error: string | null;
  activeTab: 'home' | 'config';
  platform: string;

  // Actions
  loadConfig: () => Promise<void>;
  saveConfig: (config: AppConfig) => Promise<void>;
  setAutostart: (enabled: boolean) => Promise<void>;
  setActiveTab: (tab: 'home' | 'config') => void;
  updatePort: (port: number) => Promise<void>;
  updateAutoSelectPort: (autoSelect: boolean) => Promise<void>;
  loadPlatform: () => Promise<void>;
}

export const useConfigStore = create<ConfigState>((set, get) => ({
  config: null,
  isLoading: false,
  error: null,
  activeTab: 'home',
  platform: 'unknown',

  loadPlatform: async () => {
    const { platform } = get();
    // Platform doesn't change during runtime - cache after first load
    if (platform !== 'unknown') return;

    try {
      const platformValue = await invoke<string>('get_platform');
      set((state) => ({ ...state, platform: platformValue }));
    } catch {
      set((state) => ({ ...state, platform: 'unknown' }));
    }
  },

  loadConfig: async () => {
    await executeAsync(
      {
        operation: () => invoke<AppConfig>('load_config'),
        onSuccess: (config, set) => set((state) => ({ ...state, config })),
      },
      set,
    );
  },

  saveConfig: async (config: AppConfig) => {
    await executeAsync(
      {
        operation: () => invoke('save_config', { config }),
        onSuccess: (_, set) => set((state) => ({ ...state, config })),
        errorPrefix: 'Failed to save config',
        rethrow: true,
      },
      set,
    );
  },

  setAutostart: async (enabled: boolean) => {
    await executeAsync(
      {
        operation: () => invoke('set_autostart_command', { enable: enabled }),
        onSuccess: () => {},
        errorPrefix: 'Failed to set autostart',
        rethrow: true,
      },
      set,
    );
  },

  setActiveTab: (tab: 'home' | 'config') => {
    set((state) => ({ ...state, activeTab: tab }));
  },

  updatePort: async (port: number) => {
    const { config, saveConfig } = get();
    if (!config) return;
    const newConfig = { ...config, port };
    await saveConfig(newConfig);
  },

  updateAutoSelectPort: async (autoSelect: boolean) => {
    const { config, saveConfig } = get();
    if (!config) return;
    const newConfig = { ...config, auto_select_port: autoSelect };
    await saveConfig(newConfig);
  },
}));
