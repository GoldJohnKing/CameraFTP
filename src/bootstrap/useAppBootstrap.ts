/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useEffect } from 'react';
import { useConfigStore } from '../stores/configStore';
import { usePermissionStore } from '../stores/permissionStore';
import { initializeServerEvents } from '../services/server-events';

interface UseAppBootstrapOptions {
  isMainWindow: boolean;
}

export function useAppBootstrap({ isMainWindow }: UseAppBootstrapOptions): void {
  const { loadConfig, loadPlatform, platform } = useConfigStore();
  const initializePermissions = usePermissionStore((state) => state.initialize);

  useEffect(() => {
    if (!isMainWindow) {
      return;
    }

    loadPlatform();
    initializePermissions();
    loadConfig();
  }, [initializePermissions, isMainWindow, loadConfig, loadPlatform]);

  useEffect(() => {
    if (!isMainWindow || !platform || platform === 'unknown') {
      return;
    }

    document.documentElement.className = `platform-${platform}`;
  }, [isMainWindow, platform]);

  useEffect(() => {
    if (!isMainWindow) {
      return;
    }

    let cleanupFn: (() => void) | undefined;
    let isCancelled = false;

    void initializeServerEvents()
      .then((cleanup) => {
        if (!isCancelled) {
          cleanupFn = cleanup;
        } else {
          cleanup();
        }
      })
      .catch((err) => {
        console.warn('[useAppBootstrap] Listener initialization failed:', err);
      });

    return () => {
      isCancelled = true;
      cleanupFn?.();
    };
  }, [isMainWindow]);
}
