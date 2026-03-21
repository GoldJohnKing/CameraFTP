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
  }, [isMainWindow, loadPlatform]);

  useEffect(() => {
    if (!isMainWindow) {
      return;
    }
    initializePermissions();
  }, [isMainWindow, initializePermissions]);

  useEffect(() => {
    if (!isMainWindow) {
      return;
    }
    if (platform && platform !== 'unknown') {
      document.documentElement.className = `platform-${platform}`;
    }
  }, [isMainWindow, platform]);

  useEffect(() => {
    if (!isMainWindow) {
      return;
    }

    let cleanupFn: (() => void) | undefined;
    let isCancelled = false;

    const setup = async () => {
      try {
        const cleanup = await initializeServerEvents();
        if (!isCancelled) {
          cleanupFn = cleanup;
        } else {
          cleanup();
        }
      } catch (err) {
        console.warn('[useAppBootstrap] Listener initialization failed:', err);
      }
    };

    setup();

    return () => {
      isCancelled = true;
      cleanupFn?.();
    };
  }, [isMainWindow, initializeServerEvents]);

  useEffect(() => {
    if (!isMainWindow) {
      return;
    }
    loadConfig();
  }, [isMainWindow, loadConfig]);
}
