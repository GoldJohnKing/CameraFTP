/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { listen, Event, UnlistenFn } from '@tauri-apps/api/event';

type EventHandler<T = any> = (event: Event<T>) => void;

export interface EventRegistration<T = any> {
  name: string;
  handler: EventHandler<T>;
}

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

export function createEventManager() {
  const unlisteners: UnlistenFn[] = [];
  let isCleanedUp = false;

  return {
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

    async registerAll(registrations: EventRegistration<any>[]): Promise<void> {
      if (isCleanedUp) {
        return;
      }
      const cleanup = await registerEvents(registrations);
      unlisteners.push(cleanup);
    },

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
