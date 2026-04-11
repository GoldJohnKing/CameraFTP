/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { invoke } from '@tauri-apps/api/core';
import type { Event } from '@tauri-apps/api/event';
import type { ServerInfo, ServerStateSnapshot } from '../types';

import { createEventManager, type EventRegistration } from '../utils/events';
import { requestMediaLibraryRefresh } from '../utils/gallery-refresh';
// Runtime gallery updates primarily rely on incremental events (added/deleted),
// while delete-triggered bridge refresh remains as a compatibility fallback.

import { useServerStore } from '../stores/serverStore';

function buildFallbackServerInfo(host: string, port: number): Pick<ServerInfo, 'ip' | 'url'> {
  const trimmedHost = host.trim();
  const octets = trimmedHost.split('.');
  if (octets.length !== 4) {
    return {
      ip: '127.0.0.1',
      url: `ftp://127.0.0.1:${port}`,
    };
  }

  for (const octet of octets) {
    if (!/^\d+$/.test(octet)) {
      return {
        ip: '127.0.0.1',
        url: `ftp://127.0.0.1:${port}`,
      };
    }

    const value = Number(octet);
    if (!Number.isInteger(value) || value < 0 || value > 255) {
      return {
        ip: '127.0.0.1',
        url: `ftp://127.0.0.1:${port}`,
      };
    }
  }

  const normalizedIp = octets.map((octet) => String(Number(octet))).join('.');
  return {
    ip: normalizedIp,
    url: `ftp://${normalizedIp}:${port}`,
  };
}

async function syncRuntimeStateFromBackend(): Promise<boolean> {
  try {
    const runtimeState = await invoke<{ serverInfo: ServerInfo | null; stats: ServerStateSnapshot }>('get_server_runtime_state');
    if (runtimeState.serverInfo?.isRunning) {
      useServerStore.getState().setServerRunning(runtimeState.serverInfo, {
        stats: runtimeState.stats,
      });
      return true;
    }

    useServerStore.getState().setServerStopped();
    return true;
  } catch (err) {
    console.warn('[server-events] Runtime state sync failed:', err);
    return false;
  }
}

function createEventRegistrations(): EventRegistration<any>[] {
  return [
    {
      name: 'server-started',
      handler: async (event: Event<{ ip: string; port: number }>) => {
        const { ip, port } = event.payload;
        if (await syncRuntimeStateFromBackend()) {
          return;
        }

        const normalizedServerInfo = buildFallbackServerInfo(ip, port);

        useServerStore.getState().setServerRunning({
          isRunning: true,
          ip: normalizedServerInfo.ip,
          port,
          url: normalizedServerInfo.url,
          username: 'anonymous',
          passwordInfo: '(任意密码)',
        });
      },
    },
    {
      name: 'server-stopped',
      handler: () => {
        useServerStore.getState().setServerStopped();
      },
    },
    {
      name: 'stats-update',
      handler: (event: Event<ServerStateSnapshot>) => {
        useServerStore.getState().setServerStats(event.payload);

        // Runtime upload refresh is handled via incremental gallery-items-added.
        // Full refresh is not part of the upload runtime chain.
      },
    },
    {
      name: 'media-library-refresh-requested',
      handler: () => {
        requestMediaLibraryRefresh({ reason: 'delete' });
      },
    },
    // Legacy media-store-ready runtime path remains removed.
    // Incremental gallery-items-added/gallery-items-deleted are primary, with
    // media-library-refresh-requested kept for delete compatibility bridging.
    {
      name: 'tray-start-server',
      handler: async () => {
        try {
          await useServerStore.getState().startServer();
        } catch (err) {
          console.warn('[server-events] Tray start server failed:', err);
        }
      },
    },
    {
      name: 'tray-stop-server',
      handler: async () => {
        try {
          await useServerStore.getState().stopServer();
        } catch (err) {
          console.warn('[server-events] Tray stop server failed:', err);
        }
      },
    },
  ];
}

async function syncInitialServerState(): Promise<void> {
  const synced = await syncRuntimeStateFromBackend();
  if (!synced) {
    console.warn('[server-events] Initial state sync failed');
  }
}

export async function initializeServerEvents(): Promise<() => void> {
  const eventManager = createEventManager();
  await eventManager.registerAll(createEventRegistrations());
  await syncInitialServerState();
  return () => {
    eventManager.cleanup();
  };
}
