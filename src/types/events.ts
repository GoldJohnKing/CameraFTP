/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import type { PreviewWindowConfig } from './index';

/**
 * Event payload for preview configuration changes
 * Emitted when preview settings are updated
 */
export interface ConfigChangedEvent {
    config: PreviewWindowConfig;
}

/**
 * Event payload for media store ready notification
 * Emitted when Android MediaStore has finished scanning and indexing uploaded files
 */
export type MediaStoreReadyPayload = {
  uri: string;
  relativePath: string;
  displayName: string;
  size: number;
  timestamp: number;
};
