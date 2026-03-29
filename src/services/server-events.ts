/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { invoke } from '@tauri-apps/api/core';
import type { Event } from '@tauri-apps/api/event';
import type { ServerInfo, ServerStateSnapshot } from '../types';
import { storageSettingsBridge } from '../types/global';

import { createEventManager, type EventRegistration } from '../utils/events';
// Note: scheduleMediaLibraryRefresh removed - full refresh no longer needed
// FTP uploads and deletions are handled incrementally to preserve scroll position

import { useServerStore } from '../stores/serverStore';
type ServerStartedPayload = { ip: string; port: number };

type ServerRuntimeView = {
  serverInfo: ServerInfo | null;
  stats: ServerStateSnapshot;
};

function normalizeIpv4Host(host: string): string {
  const trimmedHost = host.trim();
  const octets = trimmedHost.split('.');
  if (octets.length !== 4) {
    return '127.0.0.1';
  }

  for (const octet of octets) {
    if (!/^\d+$/.test(octet)) {
      return '127.0.0.1';
    }

    const value = Number(octet);
    if (!Number.isInteger(value) || value < 0 || value > 255) {
      return '127.0.0.1';
    }
  }

  return octets.map((octet) => String(Number(octet))).join('.');
}

function buildNormalizedIpv4ServerInfo(ip: string, port: number): Pick<ServerInfo, 'ip' | 'url'> {
  const normalizedIp = normalizeIpv4Host(ip);
  return {
    ip: normalizedIp,
    url: `ftp://${normalizedIp}:${port}`,
  };
}

async function syncRuntimeStateFromBackend(): Promise<boolean> {
  try {
    const runtimeState = await invoke<ServerRuntimeView>('get_server_runtime_state');
    if (runtimeState.serverInfo?.isRunning) {
      useServerStore.getState().setServerRunning(runtimeState.serverInfo, {
        stats: runtimeState.stats,
        immediate: true,
      });
      return true;
    }

    useServerStore.getState().setServerStopped({ immediate: true });
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
      handler: async (event: Event<ServerStartedPayload>) => {
        const { ip, port } = event.payload;
        if (await syncRuntimeStateFromBackend()) {
          return;
        }

        const normalizedServerInfo = buildNormalizedIpv4ServerInfo(ip, port);

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

        // Note: FTP upload refresh is handled incrementally via gallery-items-added event
        // to preserve scroll position. Full refresh is no longer needed here.
      },
    },
    // Note: media-store-ready and media-library-refresh-requested events are now handled
    // incrementally via gallery-items-added and gallery-items-deleted events to preserve
    // scroll position. Full refresh is no longer needed for these events.
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
    {
      name: 'android-open-manage-storage-settings',
      handler: () => {
        storageSettingsBridge.openAllFilesAccessSettings();
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
