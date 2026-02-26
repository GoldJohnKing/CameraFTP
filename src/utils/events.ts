/**
 * Tauri 事件管理器
 * 提供统一的事件注册和清理功能
 */

import { listen, Event, UnlistenFn } from '@tauri-apps/api/event';

/**
 * 事件处理器类型
 */
type EventHandler<T = unknown> = (event: Event<T>) => void;

/**
 * 事件注册配置
 */
export interface EventRegistration<T = unknown> {
  name: string;
  handler: EventHandler<T>;
}

/**
 * 批量注册事件监听器
 * 返回统一的清理函数
 */
async function registerEvents(
  registrations: EventRegistration<unknown>[]
): Promise<UnlistenFn> {
  const unlisteners: UnlistenFn[] = [];

  for (const { name, handler } of registrations) {
    try {
      const unlisten = await listen(name, handler);
      unlisteners.push(unlisten);
    } catch {
      // Silently ignore registration errors
    }
  }

  return () => {
    unlisteners.forEach((unlisten) => {
      try {
        unlisten();
      } catch {
        // Silently ignore cleanup errors
      }
    });
  };
}

/**
 * 创建事件管理器实例
 * 管理一组相关的事件监听器
 */
export function createEventManager() {
  const unlisteners: UnlistenFn[] = [];
  let isCleanedUp = false;

  return {
    /**
     * 注册单个事件
     */
    async on<T>(name: string, handler: EventHandler<T>): Promise<void> {
      if (isCleanedUp) {
        return;
      }
      try {
        const unlisten = await listen<T>(name, handler);
        unlisteners.push(unlisten);
      } catch {
        // Silently ignore registration errors
      }
    },

    /**
     * 批量注册事件
     */
    async registerAll(registrations: EventRegistration<unknown>[]): Promise<void> {
      if (isCleanedUp) {
        return;
      }
      const cleanup = await registerEvents(registrations);
      unlisteners.push(cleanup);
    },

    /**
     * 清理所有事件监听器
     */
    cleanup(): void {
      if (isCleanedUp) return;
      isCleanedUp = true;

      unlisteners.forEach((unlisten) => {
        try {
          unlisten();
        } catch {
          // Silently ignore cleanup errors
        }
      });
      unlisteners.length = 0;
    },
  };
}
