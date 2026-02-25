/**
 * Tauri 事件管理器
 * 提供统一的事件注册和清理功能
 */

import { listen, Event, UnlistenFn } from '@tauri-apps/api/event';

/**
 * 事件处理器类型
 */
export type EventHandler<T = unknown> = (event: Event<T>) => void;

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
export async function registerEvents(
  registrations: EventRegistration<unknown>[]
): Promise<UnlistenFn> {
  const unlisteners: UnlistenFn[] = [];

  for (const { name, handler } of registrations) {
    try {
      const unlisten = await listen(name, handler);
      unlisteners.push(unlisten);
    } catch (err) {
      console.error(`Failed to register event listener for '${name}':`, err);
    }
  }

  // 返回统一的清理函数
  return () => {
    unlisteners.forEach((unlisten, index) => {
      try {
        unlisten();
      } catch (err) {
        console.error(`Failed to unregister event listener #${index}:`, err);
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
        console.warn(`Cannot register event '${name}': EventManager already cleaned up`);
        return;
      }
      try {
        const unlisten = await listen<T>(name, handler);
        unlisteners.push(unlisten);
      } catch (err) {
        console.error(`Failed to register event '${name}':`, err);
      }
    },

    /**
     * 批量注册事件
     */
    async registerAll(registrations: EventRegistration<unknown>[]): Promise<void> {
      if (isCleanedUp) {
        console.warn('Cannot register events: EventManager already cleaned up');
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

      unlisteners.forEach((unlisten, index) => {
        try {
          unlisten();
        } catch (err) {
          console.error(`Failed to cleanup event listener #${index}:`, err);
        }
      });
      unlisteners.length = 0;
    },

    /**
     * 获取当前注册的监听器数量
     */
    get listenerCount(): number {
      return unlisteners.length;
    },

    /**
     * 是否已清理
     */
    get isCleanedUp(): boolean {
      return isCleanedUp;
    },
  };
}
