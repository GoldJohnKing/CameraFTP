/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import type { ServerStateSnapshot } from '../types';
import { serverStateBridge } from '../types/global';
import { retryAction } from '../utils/store';

type AndroidServerStatePayload = {
  isRunning: boolean;
  stats: ServerStateSnapshot | null;
  connectedClients: number;
};

let latestSyncRequestId = 0;
let latestRetryTimeout: ReturnType<typeof setTimeout> | null = null;

function clearRetryTimeout(timeoutId: ReturnType<typeof setTimeout> | null): void {
  if (!timeoutId) {
    return;
  }

  clearTimeout(timeoutId);

  if (latestRetryTimeout === timeoutId) {
    latestRetryTimeout = null;
  }
}

function createAndroidServerStatePayload(
  isRunning: boolean,
  stats: ServerStateSnapshot | null,
  connectedClients: number,
): AndroidServerStatePayload {
  if (!isRunning && !stats) {
    return {
      isRunning: false,
      stats: null,
      connectedClients: 0,
    };
  }

  const snapshot: ServerStateSnapshot = {
    isRunning,
    connectedClients,
    filesReceived: stats?.filesReceived ?? 0,
    bytesReceived: stats?.bytesReceived ?? 0,
    lastFile: stats?.lastFile ?? null,
  };

  return {
    isRunning,
    stats: snapshot,
    connectedClients: snapshot.connectedClients,
  };
}

function toBridgeStatsJson(stats: ServerStateSnapshot | null): string | null {
  if (!stats) {
    return null;
  }

  return JSON.stringify({
    files_transferred: stats.filesReceived || 0,
    bytes_transferred: stats.bytesReceived || 0,
  });
}

export function syncAndroidServerState(
  isRunning: boolean,
  stats: ServerStateSnapshot | null,
  connectedClients: number,
  immediate = false,
): void {
  const payload = createAndroidServerStatePayload(isRunning, stats, connectedClients);
  const requestId = ++latestSyncRequestId;
  clearRetryTimeout(latestRetryTimeout);
  let requestRetryTimeout: ReturnType<typeof setTimeout> | null = null;
  let syncSucceeded = false;

  const clearRequestRetryTimeout = (): void => {
    clearRetryTimeout(requestRetryTimeout);
    requestRetryTimeout = null;
  };

  const syncLatestPayload = (): boolean => {
    if (requestId !== latestSyncRequestId) {
      return true;
    }

    if (!serverStateBridge.isAvailable()) {
      return false;
    }

    const synced = serverStateBridge.updateState(
      payload.isRunning,
      toBridgeStatsJson(payload.stats),
      payload.connectedClients,
    );

    if (synced) {
      syncSucceeded = true;
      clearRequestRetryTimeout();
    }

    return synced;
  };

  retryAction(
    syncLatestPayload,
    { maxRetries: immediate ? 30 : 5, delayMs: immediate ? 50 : 200 },
  );

  if (syncSucceeded) {
    return;
  }

  requestRetryTimeout = setTimeout(function retryLatestPayload() {
    if (syncLatestPayload()) {
      return;
    }

    requestRetryTimeout = setTimeout(retryLatestPayload, 1000);
    latestRetryTimeout = requestRetryTimeout;
  }, immediate ? 1600 : 1200);
  latestRetryTimeout = requestRetryTimeout;
}
