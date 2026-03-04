import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { AppConfig } from '../types';
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

  // Actions
  loadConfig: () => Promise<void>;
  updateDraft: (updater: (draft: AppConfig) => AppConfig) => void;
  commitDraft: () => Promise<void>;
  resetDraft: () => void;
  setAutostart: (enabled: boolean) => Promise<void>;
  setActiveTab: (tab: 'home' | 'config') => void;
  loadPlatform: () => Promise<void>;
}

// ========== 防抖配置 ==========

const DEBOUNCE_DELAY = 100; // 统一 100ms 防抖

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

  // 创建防抖保存函数（只创建一次）
  const debouncedSave = debounce(async (config: AppConfig) => {
    await saveConfigInternal(config);
  }, DEBOUNCE_DELAY);

  return {
    config: null,
    draft: null,
    isLoading: false,
    error: null,
    activeTab: 'home',
    platform: 'unknown',

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
      debouncedSave(newDraft);
    },

    // ========== 立即保存草稿 ==========
    commitDraft: async () => {
      debouncedSave.flush();
    },

    // ========== 重置草稿 ==========
    resetDraft: () => {
      const { config } = get();
      if (config) {
        set({ draft: config });
        debouncedSave.cancel();
      }
    },

    // ========== 开机自启动 ==========
    // 注意：此操作不修改全局 isLoading，避免触发其他组件重渲染
    setAutostart: async (enabled: boolean) => {
      try {
        await invoke('set_autostart_command', { enable: enabled });
      } catch (e) {
        // Autostart is optional, but we still want to propagate the error
        throw e;
      }
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
  };
});

// ========== 选择器 Hooks（细粒度订阅）==========

/** 只订阅 draft（用于编辑界面）*/
export const useDraftConfig = () => useConfigStore(state => state.draft);

/** 只订阅 config（用于显示已保存状态）*/
export const useSavedConfig = () => useConfigStore(state => state.config);
