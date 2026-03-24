/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { describe, expect, it } from 'vitest';
import { finalizeSample } from '../gallery-telemetry';
import type { SloSample } from '../gallery-telemetry';

describe('gallery-telemetry', () => {
  const validSample: SloSample = {
    galleryOpenStart: 1000,
    galleryFirstInteractive: 1300,
    visibleThumbsExpected: 20,
    visibleThumbsReady: 19,
    scrollStop: 2000,
    viewportFullyFilled: 2200,
    deviceTier: 'mid',
    cacheMode: 'hot',
  };

  it('marks sample invalid when TTI pair is missing', () => {
    const result = finalizeSample({ ...validSample, galleryOpenStart: 0 });
    expect(result.valid).toBe(false);
    expect(result.reason).toBe('missing_tti_pair');
  });

  it('marks sample invalid when fill pair is missing', () => {
    const result = finalizeSample({ ...validSample, visibleThumbsExpected: undefined });
    expect(result.valid).toBe(false);
    expect(result.reason).toBe('missing_fill_pair');
  });

  it('marks sample invalid when scroll pair is missing', () => {
    const result = finalizeSample({ ...validSample, scrollStop: 0 });
    expect(result.valid).toBe(false);
    expect(result.reason).toBe('missing_scroll_pair');
  });

  it('computes TTI correctly for valid sample', () => {
    const result = finalizeSample(validSample);
    expect(result.valid).toBe(true);
    expect(result.ttiMs).toBe(300);
  });

  it('computes fillRate correctly for valid sample', () => {
    const result = finalizeSample(validSample);
    expect(result.valid).toBe(true);
    expect(result.fillRate).toBeCloseTo(19 / 20);
  });

  it('computes fillDelayMs correctly for valid sample', () => {
    const result = finalizeSample(validSample);
    expect(result.valid).toBe(true);
    expect(result.fillDelayMs).toBe(200);
  });
});
