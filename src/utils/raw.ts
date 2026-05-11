/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

const RAW_EXTENSIONS = new Set([
  'nef', 'nrw', 'cr2', 'cr3', 'arw', 'sr2',
  'raf', 'orf', 'rw2', 'pef', 'dng', 'x3f', 'raw', 'srw',
]);

/** Check if a filename/path has a RAW image extension. */
export function isRawFile(filenameOrPath: string): boolean {
  const ext = filenameOrPath.split('.').pop()?.toLowerCase() || '';
  return RAW_EXTENSIONS.has(ext);
}
