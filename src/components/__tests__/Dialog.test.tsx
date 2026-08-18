/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { describe, expect, it, vi } from 'vitest';
import { render } from '@testing-library/react';
import { Dialog } from '../ui/Dialog';

describe('Dialog', () => {
  it('renders its title and content when open', () => {
    const { getByText, getByTestId } = render(
      <Dialog isOpen onClose={() => {}} title="导出设置" data-testid="d">
        <button type="button">row 1</button>
      </Dialog>,
    );

    expect(getByTestId('dialog-content')).toBeTruthy();
    expect(getByText('导出设置')).toBeTruthy();
    expect(getByText('row 1')).toBeTruthy();
  });

  it('renders nothing when closed', () => {
    const { container } = render(
      <Dialog isOpen={false} onClose={() => {}} title="导出设置" data-testid="d">
        <button type="button">row 1</button>
      </Dialog>,
    );

    expect(container.querySelector('[data-testid="d"]')).toBeNull();
    expect(container.querySelector('[data-testid="dialog-content"]')).toBeNull();
    expect(container.textContent ?? '').not.toContain('row 1');
  });

  it('calls onClose when the close button is clicked', () => {
    const onClose = vi.fn();
    const { getByRole } = render(
      <Dialog isOpen onClose={onClose} title="导出设置">
        <button type="button">row 1</button>
      </Dialog>,
    );

    getByRole('button', { name: '关闭' }).click();

    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('pins the scroll-layout contract on the content element', () => {
    // 回归钉住：min-h-0 + overflow-y-auto 曾修复日期跳转选择器在受限高度
    // 容器中把对话框撑破的布局 bug。这是布局契约，不是样式噪音 —— 删除前
    // 请确认 Dialog.tsx 的滚动布局约束已由行为测试覆盖。
    const { getByTestId } = render(
      <Dialog isOpen onClose={() => {}} title="导出设置">
        <button type="button">row 1</button>
      </Dialog>,
    );

    const content = getByTestId('dialog-content');
    expect(content.className).toContain('min-h-0');
    expect(content.className).toContain('overflow-y-auto');
  });
});
