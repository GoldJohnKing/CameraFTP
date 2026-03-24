/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

export interface SloSample {
  galleryOpenStart: number;
  galleryFirstInteractive: number;
  visibleThumbsExpected: number;
  visibleThumbsReady: number;
  scrollStop: number;
  viewportFullyFilled: number;
  deviceTier: 'low' | 'mid' | 'high';
  cacheMode: 'cold' | 'hot';
}

export interface SloSampleResult {
  valid: boolean;
  reason: string | null;
  ttiMs?: number;
  fillRate?: number;
  fillDelayMs?: number;
}

export function finalizeSample(sample: Partial<SloSample>): SloSampleResult {
  // Fail-closed: if required event pair is missing, mark invalid
  if (!sample.galleryOpenStart || !sample.galleryFirstInteractive) {
    return { valid: false, reason: 'missing_tti_pair' };
  }
  if (sample.visibleThumbsExpected == null || sample.visibleThumbsReady == null) {
    return { valid: false, reason: 'missing_fill_pair' };
  }
  if (!sample.scrollStop || !sample.viewportFullyFilled) {
    return { valid: false, reason: 'missing_scroll_pair' };
  }
  // Compute metrics
  const ttiMs = sample.galleryFirstInteractive - sample.galleryOpenStart;
  const fillRate = sample.visibleThumbsReady / sample.visibleThumbsExpected;
  const fillDelayMs = sample.viewportFullyFilled - sample.scrollStop;
  return { valid: true, reason: null, ttiMs, fillRate, fillDelayMs };
}
