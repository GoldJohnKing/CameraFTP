/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { invoke } from '@tauri-apps/api/core';

export async function openExternalLink(url: string) {
  if (window.PermissionAndroid?.openExternalLink) {
    try {
      window.PermissionAndroid.openExternalLink(url);
    } catch (err) {
      console.warn('[openExternalLink] bridge call failed:', err);
    }
    return;
  }

  try {
    await invoke('open_external_link', { url });
  } catch (err) {
    console.warn('Failed to open external link:', err);
    window.open(url, '_blank');
  }
}
