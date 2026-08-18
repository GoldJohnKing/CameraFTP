/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { act } from 'react';
import { within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { flush } from '../../test-utils/flush';
import { setupReactRoot } from '../../test-utils/react-root';

const { permissionState, checkPermissionsMock, startPollingMock, stopPollingMock } = vi.hoisted(() => ({
  permissionState: { allGranted: false },
  checkPermissionsMock: vi.fn(),
  startPollingMock: vi.fn(),
  stopPollingMock: vi.fn(),
}));

vi.mock('../../stores/permissionStore', () => ({
  usePermissionStore: (selector?: (s: {
    allGranted: boolean;
    checkPermissions: () => void;
    startPolling: () => void;
    stopPolling: () => void;
  }) => unknown) =>
    selector
      ? selector({
        allGranted: permissionState.allGranted,
        checkPermissions: checkPermissionsMock,
        startPolling: startPollingMock,
        stopPolling: stopPollingMock,
      })
      : permissionState,
}));

vi.mock('../PermissionList', () => ({ PermissionList: () => <div>PermissionList</div> }));

import { PermissionDialog } from '../PermissionDialog';

describe('PermissionDialog gating', () => {
  const { getContainer, getRoot } = setupReactRoot();

  const onClose = vi.fn();
  const onAllGranted = vi.fn();

  const renderDialog = async (isOpen: boolean) => {
    await act(async () => {
      getRoot().render(
        <PermissionDialog isOpen={isOpen} onClose={onClose} onAllGranted={onAllGranted} />,
      );
      await flush();
    });
  };

  const getContinueButton = (): HTMLButtonElement => {
    const buttons = within(getContainer()).getAllByRole('button');
    const continueBtn = buttons.find((b) => b.textContent === '请授予权限' || b.textContent === '开始服务');
    if (!continueBtn) throw new Error('continue button not found');
    return continueBtn as HTMLButtonElement;
  };

  beforeEach(() => {
    permissionState.allGranted = false;
    checkPermissionsMock.mockReset();
    startPollingMock.mockReset();
    stopPollingMock.mockReset();
    onClose.mockClear();
    onAllGranted.mockClear();
  });

  it('disables the continue button until all permissions are granted', async () => {
    await renderDialog(true);

    const button = getContinueButton();
    expect(button.textContent).toBe('请授予权限');
    expect(button.disabled).toBe(true);

    permissionState.allGranted = true;
    await renderDialog(true);

    const enabled = getContinueButton();
    expect(enabled.textContent).toBe('开始服务');
    expect(enabled.disabled).toBe(false);
  });

  it('checks permissions and starts polling when opened', async () => {
    await renderDialog(true);

    expect(checkPermissionsMock).toHaveBeenCalledTimes(1);
    expect(startPollingMock).toHaveBeenCalledTimes(1);
    expect(stopPollingMock).not.toHaveBeenCalled();
  });

  it('does not check permissions or poll while closed', async () => {
    await renderDialog(false);

    expect(getContainer().textContent).toBe('');
    expect(checkPermissionsMock).not.toHaveBeenCalled();
    expect(startPollingMock).not.toHaveBeenCalled();
    expect(stopPollingMock).not.toHaveBeenCalled();
  });

  it('re-checks permissions on every open and stops polling on close', async () => {
    await renderDialog(true);
    await renderDialog(false);

    expect(stopPollingMock).toHaveBeenCalledTimes(1);

    await renderDialog(true);

    expect(checkPermissionsMock).toHaveBeenCalledTimes(2);
    expect(startPollingMock).toHaveBeenCalledTimes(2);

    await renderDialog(false);
    expect(stopPollingMock).toHaveBeenCalledTimes(2);
  });

  it('stops polling on unmount', async () => {
    await renderDialog(true);
    expect(stopPollingMock).not.toHaveBeenCalled();

    await act(async () => {
      getRoot().render(null);
      await flush();
    });

    expect(stopPollingMock).toHaveBeenCalledTimes(1);
  });

  it('fires onAllGranted and onClose when continue is clicked while granted', async () => {
    permissionState.allGranted = true;
    await renderDialog(true);

    await act(async () => {
      getContinueButton().click();
      await flush();
    });

    expect(onAllGranted).toHaveBeenCalledTimes(1);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it('does nothing when the disabled continue button is clicked', async () => {
    await renderDialog(true);

    await act(async () => {
      getContinueButton().click();
      await flush();
    });

    expect(onAllGranted).not.toHaveBeenCalled();
    expect(onClose).not.toHaveBeenCalled();
  });

  it('stops polling after the user continues (dialog closes)', async () => {
    permissionState.allGranted = true;
    await renderDialog(true);

    await act(async () => {
      getContinueButton().click();
      await flush();
    });
    expect(onClose).toHaveBeenCalledTimes(1);

    // Parent reacts to onClose by closing the dialog → cleanup stops polling.
    await renderDialog(false);
    expect(stopPollingMock).toHaveBeenCalledTimes(1);
  });
});
