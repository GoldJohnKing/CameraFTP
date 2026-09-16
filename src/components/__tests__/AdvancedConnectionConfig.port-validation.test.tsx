/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { describe, expect, it, beforeEach, vi } from 'vitest';
import { toast } from 'sonner';
import { AdvancedConnectionConfigPanel } from '../AdvancedConnectionConfig';
import type { AdvancedConnectionConfig as AdvancedConnectionConfigType } from '../../types';

const { checkPortMock, saveAuthConfigMock } = vi.hoisted(() => ({
  checkPortMock: vi.fn(),
  saveAuthConfigMock: vi.fn(),
}));

vi.mock('sonner', () => ({ toast: { error: vi.fn() } }));

vi.mock('../../hooks/usePortCheck', async () => {
  const actual = await vi.importActual<typeof import('../../hooks/usePortCheck')>('../../hooks/usePortCheck');
  return {
    ...actual,
    usePortCheck: () => ({
      checkPort: checkPortMock,
      isChecking: false,
    }),
  };
});

vi.mock('../../stores/configStore', () => ({
  useConfigStore: (selector: (state: { saveAuthConfig: typeof saveAuthConfigMock }) => unknown) => selector({
    saveAuthConfig: saveAuthConfigMock,
  }),
}));

// 与生成类型 src-tauri/bindings/AdvancedConnectionConfig.ts 对齐（无 port 字段）
const baseConfig = {
  enabled: true,
  auth: { anonymous: false, username: 'user', passwordHash: '' },
} as unknown as AdvancedConnectionConfigType;

/** 可复用的面板渲染 harness（后续端口检查等 describe 复用） */
export function renderPanel(
  overrides: Partial<Parameters<typeof AdvancedConnectionConfigPanel>[0]> = {},
) {
  return render(
    <AdvancedConnectionConfigPanel
      config={baseConfig}
      port={2121}
      platform="windows"
      isLoading={false}
      onUpdate={vi.fn()}
      {...overrides}
    />,
  );
}

describe('AdvancedConnectionConfigPanel port validation', () => {
  const onUpdate = vi.fn();

  beforeEach(() => {
    checkPortMock.mockReset();
    saveAuthConfigMock.mockReset();
    onUpdate.mockReset();
  });

  it('restores the original port when blur receives an out-of-range value', async () => {
    render(
      <AdvancedConnectionConfigPanel
        config={{
          enabled: true,
          auth: {
            anonymous: false,
            username: 'cameraftp',
            passwordHash: 'hash',
          },
        }}
        port={2121}
        platform="android"
        isLoading={false}
        onUpdate={onUpdate}
      />,
    );

    const portInput = screen.getByDisplayValue('2121');
    fireEvent.change(portInput, { target: { value: '70000' } });
    fireEvent.blur(portInput);

    expect(onUpdate).not.toHaveBeenCalled();
    expect(checkPortMock).not.toHaveBeenCalled();
    expect(screen.getByDisplayValue('2121')).toBeTruthy();
  });

  it('shows a port-in-use error when availability check fails', async () => {
    checkPortMock.mockResolvedValue({ available: false });

    render(
      <AdvancedConnectionConfigPanel
        config={{
          enabled: true,
          auth: {
            anonymous: false,
            username: 'cameraftp',
            passwordHash: 'hash',
          },
        }}
        port={2121}
        platform="android"
        isLoading={false}
        onUpdate={onUpdate}
      />,
    );

    const portInput = screen.getByDisplayValue('2121');
    fireEvent.change(portInput, { target: { value: '2233' } });
    fireEvent.blur(portInput);

    expect(await screen.findByText('端口 2233 已被占用')).toBeTruthy();
    expect(onUpdate).not.toHaveBeenCalled();
    expect(checkPortMock).toHaveBeenCalledWith(2233);
  });
});

describe('AdvancedConnectionConfigPanel password save failure', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('保存失败时 toast 报错并保持编辑态（输入不丢失）', async () => {
    renderPanel();
    const input = screen.getByPlaceholderText('输入密码');

    fireEvent.focus(input);
    fireEvent.change(input, { target: { value: 'secret123' } });
    saveAuthConfigMock.mockRejectedValueOnce(new Error('disk full'));
    fireEvent.blur(input);

    await waitFor(() =>
      expect(toast.error).toHaveBeenCalledWith(expect.stringContaining('密码保存失败')),
    );
    expect((input as HTMLInputElement).value).toBe('secret123');
  });

  it('保存成功后正常退出编辑模式', async () => {
    renderPanel();
    const input = screen.getByPlaceholderText('输入密码');

    fireEvent.focus(input);
    fireEvent.change(input, { target: { value: 'secret123' } });
    saveAuthConfigMock.mockResolvedValueOnce(undefined);
    fireEvent.blur(input);

    // 成功后退出编辑并点亮乐观标记，输入框回到占位符（而非明文/空串）
    await waitFor(() =>
      expect((input as HTMLInputElement).value).toBe('••••••••'),
    );
    expect(toast.error).not.toHaveBeenCalled();
  });
});
