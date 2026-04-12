/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

/**
 * 将字节数格式化为可读的 MB 字符串
 * @param bytes 字节数
 * @returns 格式化后的字符串，如 "12.5 MB"
 */
export function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 MB';
  const mb = bytes / (1024 * 1024);
  return `${mb.toFixed(1)} MB`;
}

/**
 * 确保异步操作至少执行指定的毫秒数，
 * 用于让 UI 动画（如 loading spinner）在快速操作中仍然可见。
 */
export async function withMinDuration<T>(fn: () => Promise<T>, minMs: number = 200): Promise<T> {
  const start = Date.now();
  try {
    return await fn();
  } finally {
    const remaining = Math.max(0, minMs - (Date.now() - start));
    if (remaining > 0) {
      await new Promise<void>((resolve) => setTimeout(resolve, remaining));
    }
  }
}
