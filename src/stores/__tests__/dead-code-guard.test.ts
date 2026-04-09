/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

describe('stores dead code guard', () => {
  it('configStore does not expose commitDraft or resetDraft', () => {
    const source = readFileSync(resolve(process.cwd(), 'src/stores/configStore.ts'), 'utf-8');

    expect(source).not.toContain('commitDraft');
    expect(source).not.toContain('resetDraft');
  });

  it('serverStore does not accept unused immediate option', () => {
    const source = readFileSync(resolve(process.cwd(), 'src/stores/serverStore.ts'), 'utf-8');

    expect(source).not.toContain('immediate');
  });

  it('serverStore does not contain redundant createStoppedStats wrapper', () => {
    const source = readFileSync(resolve(process.cwd(), 'src/stores/serverStore.ts'), 'utf-8');

    expect(source).not.toContain('createStoppedStats');
  });

  it('configStore setAutostart does not contain pointless try/catch', () => {
    const source = readFileSync(resolve(process.cwd(), 'src/stores/configStore.ts'), 'utf-8');
    const setAutostartMatch = source.match(/setAutostart[^}]+\}/s);

    expect(setAutostartMatch).toBeTruthy();
    expect(setAutostartMatch![0]).not.toContain('try');
    expect(setAutostartMatch![0]).not.toContain('catch');
  });
});
