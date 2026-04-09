/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

describe('serverStore simplification guard', () => {
  it('createRunningStats does not spread defaultStats redundantly', () => {
    const source = readFileSync(resolve(process.cwd(), 'src/stores/serverStore.ts'), 'utf-8');
    const fnMatch = source.match(/function createRunningStats[^}]+\}/s);

    expect(fnMatch).toBeTruthy();
    const fnBody = fnMatch![0];

    // Every field is explicitly set, so the spread is redundant
    expect(fnBody).not.toContain('...defaultStats');
    expect(fnBody).not.toContain('...stats');
  });

  it('setServerRunning/setServerStopped/setServerStats do not spread state redundantly', () => {
    const source = readFileSync(resolve(process.cwd(), 'src/stores/serverStore.ts'), 'utf-8');
    expect(source).not.toContain('...state');
  });
});
