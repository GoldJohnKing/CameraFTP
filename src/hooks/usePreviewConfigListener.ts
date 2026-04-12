/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import type { ConfigChangedEvent } from '../types';

export function usePreviewConfigListener(
  callback: (config: ConfigChangedEvent['config']) => void,
  enabled = true,
) {
  useEffect(() => {
    if (!enabled) return;

    const unlistenPromise = listen<ConfigChangedEvent>('preview-config-changed', (event) => {
      callback(event.payload.config);
    });

    return () => {
      void unlistenPromise.then(fn => fn()).catch(() => {});
    };
  }, [enabled, callback]);
}
