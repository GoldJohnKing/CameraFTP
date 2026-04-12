/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

/**
 * Flush pending microtasks (promises) to allow state updates to propagate.
 * Two consecutive `await Promise.resolve()` calls ensure all pending
 * microtask queue cycles are processed.
 */
export async function flush(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}
