/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, beforeEach, vi } from 'vitest';
import { AdvancedConnectionConfigPanel } from '../AdvancedConnectionConfig';

const { checkPortMock, saveAuthConfigMock } = vi.hoisted(() => ({
  checkPortMock: vi.fn(),
  saveAuthConfigMock: vi.fn(),
}));

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
