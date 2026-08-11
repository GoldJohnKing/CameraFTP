/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

/** Chinese weekday names, indexed by Date.getDay() (0 = Sunday). */
const WEEKDAYS_ZH = ['周日', '周一', '周二', '周三', '周四', '周五', '周六'] as const;

/**
 * Build a local calendar-day key (e.g. "2026-7-18") from an epoch-ms timestamp.
 * Two timestamps that fall on the same calendar day share the same key, which
 * lets the gallery group photos by day regardless of their exact capture time.
 */
export function toDateKey(ms: number): string {
  const d = new Date(ms);
  return `${d.getFullYear()}-${d.getMonth() + 1}-${d.getDate()}`;
}

/**
 * Format an epoch-ms timestamp for the gallery title, e.g. "2026年·7月18日·周三".
 * Month and day are not zero-padded, matching the requested display format.
 */
export function formatDateTitle(ms: number): string {
  const d = new Date(ms);
  return `${d.getFullYear()}年·${d.getMonth() + 1}月${d.getDate()}日·${WEEKDAYS_ZH[d.getDay()]}`;
}
