/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { describe, expect, it } from 'vitest';
import { computeGridMetrics, measureGridMetrics, DEFAULT_GRID_METRICS } from '../grid-metrics';

describe('computeGridMetrics', () => {
  // 网格实际 Tailwind 类：px-0.5(2px×2) gap-1.5(6px) pt-1(4px) pb-1.5(6px)，3 列 aspect-square。
  // 网格内容宽 = min(视口, 448) − 32（GalleryCard px-4）。
  it('390dp 视口（网格宽 358）等于旧行高 120', () => {
    expect(computeGridMetrics(358, 2, 2, 6, 6, 4, 6).pitch).toBeCloseTo(120, 5);
  });

  it('412dp 视口（网格宽 380）≈127.333', () => {
    expect(computeGridMetrics(380, 2, 2, 6, 6, 4, 6).pitch).toBeCloseTo(127 + 1 / 3, 2);
  });

  it('360dp 视口（网格宽 328）=110', () => {
    expect(computeGridMetrics(328, 2, 2, 6, 6, 4, 6).pitch).toBeCloseTo(110, 5);
  });

  it('448dp 封顶（网格宽 416）≈139.333', () => {
    expect(computeGridMetrics(416, 2, 2, 6, 6, 4, 6).pitch).toBeCloseTo(139 + 1 / 3, 2);
  });

  it('透传上下 padding', () => {
    expect(computeGridMetrics(358, 2, 2, 6, 6, 4, 6)).toEqual({ pitch: 120, padTop: 4, padBottom: 6 });
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
