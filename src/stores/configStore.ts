import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { AppConfig } from '../types';
import { formatError } from '../utils/error';

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
    try {
      const platform = await invoke<string>('get_platform');
      set((state) => ({ ...state, platform }));
    } catch (err: unknown) {
      console.error('Failed to load platform:', err);
      set((state) => ({ ...state, platform: 'unknown' }));
    }
  },

  loadConfig: async () => {
    set((state) => ({ ...state, isLoading: true, error: null }));
    try {
      const config = await invoke<AppConfig>('load_config');
      set((state) => ({ ...state, config, isLoading: false }));
    } catch (err: unknown) {
      const errorMessage = formatError(err);
      set((state) => ({ ...state, error: errorMessage || 'Failed to load config', isLoading: false }));
    }
  },

  saveConfig: async (config: AppConfig) => {
    set((state) => ({ ...state, isLoading: true, error: null }));
    try {
      await invoke('save_config', { config });
      set((state) => ({ ...state, config, isLoading: false }));
    } catch (err: unknown) {
      const errorMessage = formatError(err);
      set((state) => ({ ...state, error: errorMessage || 'Failed to save config', isLoading: false }));
      throw err;
    }
  },

  setAutostart: async (enabled: boolean) => {
    set((state) => ({ ...state, isLoading: true, error: null }));
    try {
      await invoke('set_autostart_command', { enable: enabled });
      set((state) => ({ ...state, isLoading: false }));
    } catch (err: unknown) {
      const errorMessage = formatError(err);
      set((state) => ({ ...state, error: errorMessage || 'Failed to set autostart', isLoading: false }));
      throw err;
    }
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
    const newConfig = { ...config, autoSelectPort: autoSelect };
    await saveConfig(newConfig);
  },
}));
