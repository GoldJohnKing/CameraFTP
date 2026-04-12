/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useConfigStore } from '../stores/configStore';

export function usePlatform() {
  const platform = useConfigStore((state) => state.platform);
  return {
    platform,
    isAndroid: platform === 'android',
    isWindows: platform === 'windows',
  };
}
