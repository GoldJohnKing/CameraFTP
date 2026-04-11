/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { listen, Event, UnlistenFn } from '@tauri-apps/api/event';

type EventHandler<T = unknown> = (event: Event<T>) => void;

export interface EventRegistration<T = unknown> {
  name: string;
  handler: EventHandler<T>;
}

function cleanupUnlisteners(unlisteners: UnlistenFn[]): void {
  unlisteners.forEach((unlisten) => {
    try {
      unlisten();
    } catch {
      // Silently ignore cleanup errors
    }
  });
}

export function createEventManager() {
  const unlisteners: UnlistenFn[] = [];
  let isCleanedUp = false;

  return {
    async registerAll(registrations: EventRegistration<unknown>[]): Promise<void> {
      if (isCleanedUp) {
        return;
      }

      for (const { name, handler } of registrations) {
        try {
          const unlisten = await listen(name, handler);
          unlisteners.push(unlisten);
        } catch {
          // Silently ignore registration errors
        }
      }
    },

    cleanup(): void {
      if (isCleanedUp) return;
      isCleanedUp = true;

      cleanupUnlisteners(unlisteners);
      unlisteners.length = 0;
    },
  };
}
