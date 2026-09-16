/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

export interface GridMetrics {
  /** 行顶到下一行顶的垂直距离（单元格高度 + 行间距），px */
  pitch: number;
  /** 网格内部上 padding（首行偏移），px */
  padTop: number;
  /** 网格内部下 padding，px */
  padBottom: number;
}

/** 测量前的回退值（同时是 jsdom 测试环境下的稳定值，保持旧行为） */
export const DEFAULT_GRID_METRICS: GridMetrics = { pitch: 120, padTop: 4, padBottom: 6 };

/**
 * 3 列 aspect-square 网格的纯几何计算：
 * 单元格宽 = (contentWidth − 左右 padding − 2 × 列间距) / 3
 * pitch    = 单元格高（= 宽） + 行间距
 */
export function computeGridMetrics(
  contentWidth: number,
  padLeft: number,
  padRight: number,
  colGap: number,
  rowGap: number,
  padTop: number,
  padBottom: number,
): GridMetrics {
  const cellWidth = (contentWidth - padLeft - padRight - 2 * colGap) / 3;
  return { pitch: cellWidth + rowGap, padTop, padBottom };
}

/**
 * 从真实 DOM 元素读取计算样式并得出行距。
 * 用 getComputedStyle().width（浏览器返回布局后的 used value），
 * 而非 getBoundingClientRect（jsdom 下恒为 0，内联样式可被 computed style 解析）。
 */
export function measureGridMetrics(el: HTMLElement): GridMetrics {
  const cs = window.getComputedStyle(el);
  const num = (v: string): number => parseFloat(v) || 0;
  const width = num(cs.width);
  if (!(width > 0)) {
    return DEFAULT_GRID_METRICS;
  }
  return computeGridMetrics(
    width,
    num(cs.paddingLeft),
    num(cs.paddingRight),
    num(cs.columnGap),
    num(cs.rowGap),
    num(cs.paddingTop),
    num(cs.paddingBottom),
  );
}
