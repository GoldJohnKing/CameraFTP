import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { AppConfig, PreviewWindowConfig } from '../types';
import { executeAsync } from '../utils/store';

// ========== 防抖工具 ==========

/**
 * 防抖函数，支持取消和立即执行
 */
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

// ========== Store 接口定义 ==========

interface ConfigState {
  // 已持久化的配置（真实来源）
  config: AppConfig | null;
  // 共享草稿（所有界面编辑目标）
  draft: AppConfig | null;
  // 状态
  isLoading: boolean;
  error: string | null;
  activeTab: 'home' | 'config';
  platform: string;
  previewConfig: PreviewWindowConfig | null;

  // Actions
  loadConfig: () => Promise<void>;
  updateDraft: (updater: (draft: AppConfig) => AppConfig) => void;
  commitDraft: () => Promise<void>;
  resetDraft: () => void;
  setAutostart: (enabled: boolean) => Promise<void>;
  setActiveTab: (tab: 'home' | 'config') => void;
  loadPlatform: () => Promise<void>;
  loadPreviewConfig: () => Promise<void>;
  updatePreviewConfig: (config: PreviewWindowConfig) => Promise<void>;
}

// ========== 防抖保存（模块级单例）==========

const DEBOUNCE_DELAY = 100; // 统一 100ms 防抖

let debouncedSave: ReturnType<typeof debounce> | null = null;

const getOrCreateDebouncedSave = (
  saveFn: (config: AppConfig) => Promise<void>
) => {
  if (!debouncedSave) {
    debouncedSave = debounce(async (config: AppConfig) => {
      await saveFn(config);
    }, DEBOUNCE_DELAY);
  }
  return debouncedSave;
};

// ========== Store 实现 ==========

export const useConfigStore = create<ConfigState>((set, get) => {
  // 内部保存方法
  const saveConfigInternal = async (config: AppConfig) => {
    try {
      await invoke('save_config', { config });
      set({ config, error: null });
    } catch (e) {
      set({ error: String(e) });
      throw e;
    }
  };

  return {
    config: null,
    draft: null,
    isLoading: false,
    error: null,
    activeTab: 'home',
    platform: 'unknown',
    previewConfig: null,

    // ========== 加载配置 ==========
    loadConfig: async () => {
      await executeAsync(
        {
          operation: () => invoke<AppConfig>('load_config'),
          onSuccess: (config, set) => set((state) => ({ ...state, config, draft: config })),
        },
        set,
      );
    },

    // ========== 更新草稿（核心方法）==========
    updateDraft: (updater: (draft: AppConfig) => AppConfig) => {
      const { draft } = get();
      if (!draft) return;

      const newDraft = updater(draft);
      set({ draft: newDraft });

      // 触发防抖保存
      const debounced = getOrCreateDebouncedSave(saveConfigInternal);
      debounced(newDraft);
    },

    // ========== 立即保存草稿 ==========
    commitDraft: async () => {
      const debounced = getOrCreateDebouncedSave(saveConfigInternal);
      debounced.flush();
    },

    // ========== 重置草稿 ==========
    resetDraft: () => {
      const { config } = get();
      if (config) {
        set({ draft: config });
        const debounced = getOrCreateDebouncedSave(saveConfigInternal);
        debounced.cancel();
      }
    },

    // ========== 开机自启动 ==========
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

    // ========== Tab 切换 ==========
    setActiveTab: (tab: 'home' | 'config') => {
      set({ activeTab: tab });
    },

    // ========== 平台检测 ==========
    loadPlatform: async () => {
      const { platform } = get();
      if (platform !== 'unknown') return;

      try {
        const platformValue = await invoke<string>('get_platform');
        set({ platform: platformValue });
      } catch {
        set({ platform: 'unknown' });
      }
    },

    // ========== 预览配置 ==========
    loadPreviewConfig: async () => {
      try {
        const config = await invoke<PreviewWindowConfig>('get_preview_config');
        set({ previewConfig: config });
      } catch (error) {
        console.error('Failed to load preview config:', error);
      }
    },

    updatePreviewConfig: async (config: PreviewWindowConfig) => {
      try {
        await invoke('set_preview_config', { config });
        set({ previewConfig: config });
      } catch (error) {
        console.error('Failed to update preview config:', error);
        throw error;
      }
    },
  };
});

// ========== 选择器 Hooks（细粒度订阅）==========

/** 只订阅 draft（用于编辑界面）*/
export const useDraftConfig = () => useConfigStore(state => state.draft);

/** 只订阅 config（用于显示已保存状态）*/
export const useSavedConfig = () => useConfigStore(state => state.config);

/** 订阅是否有未保存的更改 */
export const useHasUnsavedChanges = () => {
  const config = useConfigStore(state => state.config);
  const draft = useConfigStore(state => state.draft);
  if (!config || !draft) return false;
  return JSON.stringify(config) !== JSON.stringify(draft);
};
