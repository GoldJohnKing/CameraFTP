/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import type { ServerStateSnapshot } from '../types';
import { serverStateBridge } from '../types/global';
import { retryAction } from '../utils/store';

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
  retryAction(
    () => {
      if (!serverStateBridge.isAvailable()) {
        return false;
      }

      return serverStateBridge.updateState(
        isRunning,
        toBridgeStatsJson(stats),
        connectedClients,
      );
    },
    { maxRetries: immediate ? 30 : 5, delayMs: immediate ? 50 : 200 },
  );
}
