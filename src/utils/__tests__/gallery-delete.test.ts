/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { describe, expect, it } from 'vitest';
import { buildDeleteFailureMessage } from '../gallery-delete';

describe('gallery-delete', () => {
  it('returns a visible message when all selected images fail to delete', () => {
    expect(buildDeleteFailureMessage({
      deleted: [],
      notFound: [],
      failed: ['content://media/1', 'content://media/2'],
    })).toContain('2');
  });

  it('returns null when at least one image was removed', () => {
    expect(buildDeleteFailureMessage({
      deleted: ['content://media/1'],
      notFound: [],
      failed: ['content://media/2'],
    })).toBeNull();
  });
});
