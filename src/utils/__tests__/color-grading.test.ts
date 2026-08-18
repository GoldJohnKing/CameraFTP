/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { describe, expect, it } from 'vitest';
import { applyColorGradingLastUsed } from '../color-grading';
import type { AppConfig, ColorGradingLastUsed } from '../../types';

const LAST_USED: ColorGradingLastUsed = {
  presetId: 'fujifilm-velvia',
  evOffset: -0.7,
  meteringMode: 'spot',
};

/** Minimal draft with distinct old values so every mirrored field is observable. */
function makeDraft(): AppConfig {
  return {
    autoColorGrading: {
      enabled: false,
      presetId: 'fujifilm-provia',
      evOffset: 0,
      meteringMode: 'matrix',
    },
    colorGradingLastUsed: {
      presetId: 'kodak-portra',
      evOffset: 1.2,
      meteringMode: 'center',
    },
    savePath: '/tmp/photos',
    port: 2121,
  } as unknown as AppConfig;
}

describe('applyColorGradingLastUsed', () => {
  it('records the last-used parameters on the draft', () => {
    const result = applyColorGradingLastUsed(makeDraft(), LAST_USED, false);

    expect(result.colorGradingLastUsed).toEqual(LAST_USED);
  });

  it('leaves autoColorGrading untouched when syncToAuto is false', () => {
    const draft = makeDraft();

    const result = applyColorGradingLastUsed(draft, LAST_USED, false);

    expect(result.autoColorGrading).toEqual({
      enabled: false,
      presetId: 'fujifilm-provia',
      evOffset: 0,
      meteringMode: 'matrix',
    });
  });

  it('mirrors preset/metering/EV into autoColorGrading while preserving enabled', () => {
    const result = applyColorGradingLastUsed(makeDraft(), LAST_USED, true);

    expect(result.autoColorGrading).toEqual({
      enabled: false, // preserved — not reset or forced true
      presetId: 'fujifilm-velvia',
      evOffset: -0.7,
      meteringMode: 'spot',
    });
  });

  it('preserves enabled: true when mirroring into autoColorGrading', () => {
    const draft = makeDraft();
    (draft.autoColorGrading as { enabled: boolean }).enabled = true;

    const result = applyColorGradingLastUsed(draft, LAST_USED, true);

    expect(result.autoColorGrading).toEqual({
      enabled: true,
      presetId: 'fujifilm-velvia',
      evOffset: -0.7,
      meteringMode: 'spot',
    });
  });

  it('is a no-op on autoColorGrading when it is null, even with syncToAuto', () => {
    const draft = makeDraft();
    draft.autoColorGrading = null;

    const result = applyColorGradingLastUsed(draft, LAST_USED, true);

    expect(result.autoColorGrading).toBeNull();
    expect(result.colorGradingLastUsed).toEqual(LAST_USED);
  });

  it('returns a new draft without mutating the input', () => {
    const draft = makeDraft();
    const draftSnapshot = JSON.stringify(draft);

    const result = applyColorGradingLastUsed(draft, LAST_USED, true);

    expect(result).not.toBe(draft);
    expect(JSON.stringify(draft)).toBe(draftSnapshot);
    // The mirrored auto config is a copy, not the original object.
    expect(result.autoColorGrading).not.toBe(draft.autoColorGrading);
  });

  it('preserves unrelated top-level config fields', () => {
    const result = applyColorGradingLastUsed(makeDraft(), LAST_USED, true);

    expect(result.port).toBe(2121);
    expect(result.savePath).toBe('/tmp/photos');
  });
});
