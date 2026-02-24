import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { AppConfig } from '../types';

interface ConfigState {
  config: AppConfig | null;
  isLoading: boolean;
  error: string | null;
  activeTab: 'home' | 'config';
  platform: string;
  
  // Actions
  loadConfig: () => Promise<void>;
  saveConfig: (config: AppConfig) => Promise<void>;
  updateSavePath: (path: string) => Promise<void>;
  setAutostart: (enabled: boolean) => Promise<void>;
  setActiveTab: (tab: 'home' | 'config') => void;
  selectDirectory: () => Promise<string | null>;
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
      set({ platform });
    } catch (err) {
      console.error('Failed to load platform:', err);
      set({ platform: 'unknown' });
    }
  },

  loadConfig: async () => {
    set({ isLoading: true, error: null });
    try {
      const config = await invoke<AppConfig>('load_config');
      set({ config, isLoading: false });
    } catch (err: any) {
      set({ error: err?.message || 'Failed to load config', isLoading: false });
    }
  },

  saveConfig: async (config: AppConfig) => {
    set({ isLoading: true, error: null });
    try {
      await invoke('save_config', { config });
      set({ config, isLoading: false });
    } catch (err: any) {
      set({ error: err?.message || 'Failed to save config', isLoading: false });
      throw err;
    }
  },

  updateSavePath: async (path: string) => {
    const { config, saveConfig } = get();
    if (!config) return;
    const newConfig = { ...config, save_path: path };
    await saveConfig(newConfig);
  },

  setAutostart: async (enabled: boolean) => {
    set({ isLoading: true, error: null });
    try {
      await invoke('set_autostart_command', { enable: enabled });
      set({ isLoading: false });
    } catch (err: any) {
      set({ error: err?.message || 'Failed to set autostart', isLoading: false });
      throw err;
    }
  },

  setActiveTab: (tab: 'home' | 'config') => {
    set({ activeTab: tab });
  },

  selectDirectory: async () => {
    try {
      const selected = await invoke<string | null>('select_save_directory');
      return selected;
    } catch (err) {
      console.error('Failed to select directory:', err);
      return null;
    }
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
