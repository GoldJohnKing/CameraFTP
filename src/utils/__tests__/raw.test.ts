/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { describe, expect, it } from 'vitest';
import { isRawFile } from '../raw';

describe('isRawFile', () => {
  const rawExtensions = ['nef', 'nrw', 'cr2', 'cr3', 'arw', 'sr2', 'raf', 'orf', 'rw2', 'pef', 'dng', 'x3f', 'raw', 'srw'];

  it.each(rawExtensions)('recognizes .%s as RAW', (ext) => {
    expect(isRawFile(`photo.${ext}`)).toBe(true);
  });

  it.each(['jpg', 'jpeg', 'png', 'mp4', 'txt', 'gif'])('rejects .%s as non-RAW', (ext) => {
    expect(isRawFile(`photo.${ext}`)).toBe(false);
  });

  it('is case insensitive', () => {
    expect(isRawFile('photo.NEF')).toBe(true);
    expect(isRawFile('photo.Cr2')).toBe(true);
  });

  it('handles full paths', () => {
    expect(isRawFile('/storage/photos/DCIM/IMG_001.ARW')).toBe(true);
  });

  it('handles dot-only filenames', () => {
    expect(isRawFile('.nef')).toBe(false);
  });

  it('handles no extension', () => {
    expect(isRawFile('photo')).toBe(false);
  });

  it('handles empty string', () => {
    expect(isRawFile('')).toBe(false);
  });

  it('handles multiple dots (uses last dot)', () => {
    expect(isRawFile('photo.backup.nef')).toBe(true);
  });
});
