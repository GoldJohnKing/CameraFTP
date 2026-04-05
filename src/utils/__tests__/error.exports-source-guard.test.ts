/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { describe, expect, it } from 'vitest';
import * as errorUtils from '../error';
import * as typeExports from '../../types';

describe('cleanup export surface', () => {
  it('does not expose removed dead helpers', () => {
    expect('ignoreErrors' in errorUtils).toBe(false);
    expect('isPermissionAndroidAvailable' in typeExports).toBe(false);
  });
});
