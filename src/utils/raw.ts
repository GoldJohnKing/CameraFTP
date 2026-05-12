/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

const RAW_EXTENSIONS = new Set([
  'nef', 'nrw', 'cr2', 'cr3', 'arw', 'sr2',
  'raf', 'orf', 'rw2', 'pef', 'dng', 'x3f', 'raw', 'srw',
]);
// NOTE: Keep in sync with src-tauri/src/image_utils.rs RAW_EXTENSIONS (Rust side).

/** Check if a filename/path has a RAW image extension. */
export function isRawFile(filenameOrPath: string): boolean {
  // Use lastIndexOf to extract extension, consistent with Rust's Path::extension()
  // which returns None for dot-only filenames like ".nef"
  const dotIndex = filenameOrPath.lastIndexOf('.');
  if (dotIndex <= 0) return false; // no extension or dot-only filename
  const ext = filenameOrPath.slice(dotIndex + 1).toLowerCase();
  return RAW_EXTENSIONS.has(ext);
}
