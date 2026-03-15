/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

export function shouldScheduleUploadRefresh(previousFilesReceived: number, nextFilesReceived: number): boolean {
  return nextFilesReceived > previousFilesReceived;
}
