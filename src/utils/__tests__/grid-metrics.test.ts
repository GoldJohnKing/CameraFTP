/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { describe, expect, it } from 'vitest';
import { computeGridMetrics, sameGridMetrics, measureGridMetrics, DEFAULT_GRID_METRICS } from '../grid-metrics';

/** 真实网格的 Tailwind 庞性能参数：px-0.5(2px×2) gap-1.5(6px) pt-1(4px) pb-1.5(6px) */
const REAL_GRID = { padLeft: 2, padRight: 2, colGap: 6, rowGap: 6, padTop: 4, padBottom: 6 };

describe('computeGridMetrics', () => {
  // 网格实际 Tailwind 类：px-0.5(2px×2) gap-1.5(6px) pt-1(4px) pb-1.5(6px)，3 列 aspect-square。
  // 网格内容宽 = min(视口, 448) − 32（GalleryCard px-4）。
  it('390dp 视口（网格宽 358）等于旧行高 120', () => {
    expect(computeGridMetrics({ width: 358, ...REAL_GRID }).pitch).toBeCloseTo(120, 5);
  });

  it('412dp 视口（网格宽 380）≈127.333', () => {
    expect(computeGridMetrics({ width: 380, ...REAL_GRID }).pitch).toBeCloseTo(127 + 1 / 3, 2);
  });

  it('360dp 视口（网格宽 328）=110', () => {
    expect(computeGridMetrics({ width: 328, ...REAL_GRID }).pitch).toBeCloseTo(110, 5);
  });

  it('448dp 封顶（网格宽 416）≈139.333', () => {
    expect(computeGridMetrics({ width: 416, ...REAL_GRID }).pitch).toBeCloseTo(139 + 1 / 3, 2);
  });

  it('透传上下 padding', () => {
    expect(computeGridMetrics({ width: 358, ...REAL_GRID })).toEqual({ pitch: 120, padTop: 4, padBottom: 6 });
  });
});

describe('sameGridMetrics', () => {
  it('完全相同 → true', () => {
    expect(sameGridMetrics(DEFAULT_GRID_METRICS, { ...DEFAULT_GRID_METRICS })).toBe(true);
  });

  it('各字段差 ≤ 0.5px（亚像素抖动）→ true', () => {
    const jittered = {
      pitch: DEFAULT_GRID_METRICS.pitch + 0.5,
      padTop: DEFAULT_GRID_METRICS.padTop - 0.5,
      padBottom: DEFAULT_GRID_METRICS.padBottom + 0.25,
    };
    expect(sameGridMetrics(DEFAULT_GRID_METRICS, jittered)).toBe(true);
  });

  it('任一字段差 > 0.5px → false', () => {
    expect(
      sameGridMetrics(DEFAULT_GRID_METRICS, { ...DEFAULT_GRID_METRICS, pitch: DEFAULT_GRID_METRICS.pitch + 0.51 }),
    ).toBe(false);
    expect(
      sameGridMetrics(DEFAULT_GRID_METRICS, { ...DEFAULT_GRID_METRICS, padTop: DEFAULT_GRID_METRICS.padTop + 1 }),
    ).toBe(false);
    expect(
      sameGridMetrics(DEFAULT_GRID_METRICS, { ...DEFAULT_GRID_METRICS, padBottom: DEFAULT_GRID_METRICS.padBottom - 0.6 }),
    ).toBe(false);
  });
});

describe('measureGridMetrics', () => {
  it('从内联样式读取（jsdom 可解析内联样式）', () => {
    const el = document.createElement('div');
    el.style.cssText = 'width:380px;padding:4px 2px 6px;column-gap:6px;row-gap:6px';
    expect(measureGridMetrics(el).pitch).toBeCloseTo(127 + 1 / 3, 2);
  });

  it('宽度不可解析（jsdom 无布局/未测量）时回退默认值', () => {
    const el = document.createElement('div');
    expect(measureGridMetrics(el)).toEqual(DEFAULT_GRID_METRICS);
  });
});
