/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { describe, expect, it } from 'vitest';
import { classifyFile } from '../gallery-filter';
import type { MediaItemDto } from '../../types';

/** Build a MediaItemDto whose extension is derived from the given field. */
function item(opts: Partial<Pick<MediaItemDto, 'filePath' | 'displayName' | 'uri'>>): MediaItemDto {
  return {
    mediaId: 'm',
    uri: opts.uri ?? '',
    dateModifiedMs: 0,
    width: null,
    height: null,
    mimeType: null,
    displayName: opts.displayName ?? null,
    filePath: opts.filePath ?? null,
  };
}

// getFileExtension is module-private; its fallback chain is observed through
// classifyFile, using fixtures whose categories prove which field was read.
describe('extension extraction fallback chain (via classifyFile)', () => {
  it('reads the extension from filePath first', () => {
    expect(classifyFile(item({ filePath: '/a/b/Photo.JPG', displayName: 'x.PNG', uri: 'y.heic' }))).toBe('jpeg');
  });

  it('falls back to displayName when filePath is null', () => {
    expect(classifyFile(item({ displayName: 'Shot.Heic' }))).toBe('heif');
  });

  it('falls back to uri when filePath and displayName are null', () => {
    expect(classifyFile(item({ uri: 'content://media/IMG_001.CR3' }))).toBe('raw');
  });

  it('returns empty string when no field has an extension', () => {
    expect(classifyFile(item({ uri: 'content://media/noext' }))).toBe('other');
  });

  it('ignores a leading-dot-only name (no real extension)', () => {
    expect(classifyFile(item({ filePath: '.nef' }))).toBe('other');
  });
});

describe('classifyFile', () => {
  it('groups jpg and jpeg into the jpeg category, case-insensitively', () => {
    expect(classifyFile(item({ filePath: 'a.jpg' }))).toBe('jpeg');
    expect(classifyFile(item({ filePath: 'a.JPEG' }))).toBe('jpeg');
    expect(classifyFile(item({ filePath: 'a.JpG' }))).toBe('jpeg');
  });

  it('groups heif, hif and heic into the heif category', () => {
    expect(classifyFile(item({ filePath: 'a.heif' }))).toBe('heif');
    expect(classifyFile(item({ filePath: 'a.HIF' }))).toBe('heif');
    expect(classifyFile(item({ filePath: 'a.heic' }))).toBe('heif');
  });

  it('classifies known RAW extensions as raw', () => {
    expect(classifyFile(item({ filePath: 'a.nef' }))).toBe('raw');
    expect(classifyFile(item({ filePath: 'a.CR3' }))).toBe('raw');
    expect(classifyFile(item({ filePath: 'a.dng' }))).toBe('raw');
    expect(classifyFile(item({ filePath: 'a.arw' }))).toBe('raw');
  });

  it('falls back to other for unrecognised or missing extensions', () => {
    expect(classifyFile(item({ filePath: 'a.png' }))).toBe('other');
    expect(classifyFile(item({ filePath: 'a.tiff' }))).toBe('other');
    expect(classifyFile(item({ uri: 'content://media/noext' }))).toBe('other');
  });
});
