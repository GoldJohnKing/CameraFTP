/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { describe, expect, it } from 'vitest';
import { formatDateTitle, toDateKey } from '../date-format';

/** Build a local-time epoch-ms timestamp (month is 1-based, like the display). */
function localMs(
  year: number,
  month: number,
  day: number,
  hour = 0,
  minute = 0,
  second = 0,
  ms = 0,
): number {
  return new Date(year, month - 1, day, hour, minute, second, ms).getTime();
}

describe('toDateKey', () => {
  it('maps every time within one calendar day to the same key', () => {
    const dayStart = localMs(2026, 7, 18, 0, 0, 0, 0);
    const midDay = localMs(2026, 7, 18, 12, 30, 5, 123);
    const dayEnd = localMs(2026, 7, 18, 23, 59, 59, 999);

    expect(toDateKey(dayStart)).toBe('2026-7-18');
    expect(toDateKey(midDay)).toBe(toDateKey(dayStart));
    expect(toDateKey(dayEnd)).toBe(toDateKey(dayStart));
  });

  it('uses a different key for the first instant of the next day (day boundary)', () => {
    const lastInstant = localMs(2026, 7, 18, 23, 59, 59, 999);
    const firstInstantOfNextDay = localMs(2026, 7, 19, 0, 0, 0, 0);

    expect(toDateKey(lastInstant)).toBe('2026-7-18');
    expect(toDateKey(firstInstantOfNextDay)).toBe('2026-7-19');
    expect(toDateKey(firstInstantOfNextDay)).not.toBe(toDateKey(lastInstant));
  });

  it('rolls over at the month boundary (Jan 31 → Feb 1) without zero-padding', () => {
    expect(toDateKey(localMs(2026, 1, 31, 23, 59, 59, 999))).toBe('2026-1-31');
    expect(toDateKey(localMs(2026, 2, 1))).toBe('2026-2-1');
  });

  it('rolls over at the year boundary (Dec 31 → Jan 1)', () => {
    expect(toDateKey(localMs(2025, 12, 31, 23, 59, 59, 999))).toBe('2025-12-31');
    expect(toDateKey(localMs(2026, 1, 1))).toBe('2026-1-1');
    expect(toDateKey(localMs(2026, 1, 1))).not.toBe(toDateKey(localMs(2025, 12, 31, 23, 59, 59, 999)));
  });
});

describe('formatDateTitle', () => {
  it('formats a fixed Saturday with the CN weekday name (2026-07-18 is 周六)', () => {
    expect(formatDateTitle(localMs(2026, 7, 18))).toBe('2026年·7月18日·周六');
  });

  it('maps Sunday to 周日 (2026-07-19)', () => {
    expect(formatDateTitle(localMs(2026, 7, 19))).toBe('2026年·7月19日·周日');
  });

  it('maps New Year\'s Day 2026 (a Thursday) to 周四 without zero-padding', () => {
    expect(formatDateTitle(localMs(2026, 1, 1))).toBe('2026年·1月1日·周四');
  });
});
