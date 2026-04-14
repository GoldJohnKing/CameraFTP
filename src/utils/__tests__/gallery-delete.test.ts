/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { describe, expect, it } from 'vitest';
import { buildDeleteFailureMessage } from '../gallery-delete';

describe('gallery-delete', () => {
  it('returns null when at least one image was deleted', () => {
    expect(buildDeleteFailureMessage({
      deleted: ['content://media/1'],
      notFound: [],
      failed: ['content://media/2'],
    })).toBeNull();
  });

  it('returns null when at least one image was not found', () => {
    expect(buildDeleteFailureMessage({
      deleted: [],
      notFound: ['content://media/1'],
      failed: ['content://media/2'],
    })).toBeNull();
  });

  it('returns null when nothing failed', () => {
    expect(buildDeleteFailureMessage({
      deleted: ['content://media/1'],
      notFound: [],
      failed: [],
    })).toBeNull();
  });

  it('returns failure message when all images failed to delete', () => {
    const message = buildDeleteFailureMessage({
      deleted: [],
      notFound: [],
      failed: ['content://media/1', 'content://media/2'],
    });
    expect(message).toContain('2 张图片未能删除');
    expect(message).toContain('删除失败');
  });

});
