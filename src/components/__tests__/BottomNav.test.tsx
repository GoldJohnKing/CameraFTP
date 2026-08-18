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

/**
 * Vitest runs in Node, but @types/node is not installed, so `process` needs a
 * typed cast to subscribe to unhandled rejection notifications.
 */
const rejectionTracker = (globalThis as {
  process?: {
    on: (event: 'unhandledRejection', listener: (reason: unknown) => void) => void;
    off: (event: 'unhandledRejection', listener: (reason: unknown) => void) => void;
  };
}).process;

const { invokeMock, setActiveTabMock, configState } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  setActiveTabMock: vi.fn(),
  configState: {
    activeTab: 'home' as string,
    platform: 'android' as string,
    setActiveTab: (tab: string) => setActiveTabMock(tab),
  },
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

vi.mock('../../stores/configStore', () => ({
  useConfigStore: (selector?: (s: typeof configState) => unknown) =>
    selector ? selector(configState) : configState,
}));

import { BottomNav } from '../BottomNav';

describe('BottomNav platform behavior', () => {
  const { getContainer, getRoot } = setupReactRoot();

  const renderNav = async () => {
    await act(async () => {
      getRoot().render(<BottomNav />);
      await flush();
    });
  };

  const getTabButton = (label: string): HTMLButtonElement => {
    return within(getContainer())
      .getAllByRole('button')
      .find((b) => b.textContent?.includes(label)) as HTMLButtonElement;
  };

  const clickTab = async (label: string) => {
    await act(async () => {
      getTabButton(label).click();
      await flush();
    });
  };

  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(undefined);
    setActiveTabMock.mockReset();
    configState.activeTab = 'home';
    configState.platform = 'android';
  });

  it('switches to the gallery tab on Android', async () => {
    await renderNav();

    await clickTab('图库');

    expect(setActiveTabMock).toHaveBeenCalledWith('gallery');
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it('switches tabs for home and config on Android', async () => {
    await renderNav();

    await clickTab('主页');
    expect(setActiveTabMock).toHaveBeenCalledWith('home');

    await clickTab('配置');
    expect(setActiveTabMock).toHaveBeenCalledWith('config');

    expect(invokeMock).not.toHaveBeenCalled();
  });

  it('opens the save directory instead of switching tabs on desktop', async () => {
    configState.platform = 'windows';
    await renderNav();

    await clickTab('图库');

    expect(invokeMock).toHaveBeenCalledWith('open_save_directory');
    expect(setActiveTabMock).not.toHaveBeenCalledWith('gallery');
  });

  it('silently ignores a failing open_save_directory invoke on desktop', async () => {
    const unhandledRejection = vi.fn();
    rejectionTracker?.on('unhandledRejection', unhandledRejection);

    try {
      configState.platform = 'windows';
      invokeMock.mockRejectedValue(new Error('directory unavailable'));
      await renderNav();

      await clickTab('图库');

      expect(invokeMock).toHaveBeenCalledWith('open_save_directory');
      expect(unhandledRejection).not.toHaveBeenCalled();
      expect(setActiveTabMock).not.toHaveBeenCalledWith('gallery');
    } finally {
      rejectionTracker?.off('unhandledRejection', unhandledRejection);
    }
  });
});
