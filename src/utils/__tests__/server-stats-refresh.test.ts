/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { describe, expect, it } from 'vitest';
import { shouldScheduleUploadRefresh } from '../server-stats-refresh';

describe('server-stats-refresh', () => {
  it('returns true when filesReceived increases', () => {
    expect(shouldScheduleUploadRefresh(10, 11)).toBe(true);
  });

  it('returns false when filesReceived does not increase', () => {
    expect(shouldScheduleUploadRefresh(10, 10)).toBe(false);
    expect(shouldScheduleUploadRefresh(10, 9)).toBe(false);
  });
});
