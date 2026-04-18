/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { describe, expect, it } from 'vitest';
import { parsePortInput } from '../usePortCheck';

describe('parsePortInput', () => {
  it('returns empty for whitespace-only input', () => {
    expect(parsePortInput('   ', 1, 65535)).toEqual({
      valid: false,
      reason: 'empty',
    });
  });

  it('returns empty for empty string', () => {
    expect(parsePortInput('', 1, 65535)).toEqual({
      valid: false,
      reason: 'empty',
    });
  });

  it('returns invalid_number for non-numeric input', () => {
    expect(parsePortInput('abc', 1, 65535)).toEqual({
      valid: false,
      reason: 'invalid_number',
    });
  });

  it('returns invalid_number for port 0', () => {
    expect(parsePortInput('0', 1, 65535)).toEqual({
      valid: false,
      reason: 'invalid_number',
    });
  });

  it('returns invalid_number for negative port', () => {
    expect(parsePortInput('-1', 1, 65535)).toEqual({
      valid: false,
      reason: 'invalid_number',
    });
  });

  it('returns invalid_number for port above 65535', () => {
    expect(parsePortInput('65536', 1, 65535)).toEqual({
      valid: false,
      reason: 'invalid_number',
    });
  });

  it('returns out_of_range when below min', () => {
    expect(parsePortInput('80', 1025, 65535)).toEqual({
      valid: false,
      reason: 'out_of_range',
    });
  });

  it('returns out_of_range when above max', () => {
    expect(parsePortInput('100', 1, 80)).toEqual({
      valid: false,
      reason: 'out_of_range',
    });
  });

  it('returns valid for port 21', () => {
    expect(parsePortInput('21', 1, 65535)).toEqual({
      valid: true,
      port: 21,
    });
  });

  it('returns valid at min boundary', () => {
    expect(parsePortInput('1025', 1025, 65535)).toEqual({
      valid: true,
      port: 1025,
    });
  });

  it('returns valid at max boundary', () => {
    expect(parsePortInput('65535', 1, 65535)).toEqual({
      valid: true,
      port: 65535,
    });
  });

  it('returns valid for port 1 (lower absolute bound)', () => {
    expect(parsePortInput('1', 1, 65535)).toEqual({
      valid: true,
      port: 1,
    });
  });

  it('trims whitespace before parsing', () => {
    expect(parsePortInput('  2121  ', 1, 65535)).toEqual({
      valid: true,
      port: 2121,
    });
  });
});
