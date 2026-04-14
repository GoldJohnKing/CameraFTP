/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { act } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { usePreviewNavigation } from '../usePreviewNavigation';
import { flush } from '../../test-utils/flush';
import { setupReactRoot } from '../../test-utils/react-root';

const { invokeMock, listenMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  listenMock: vi.fn(),
}));

let fileIndexChangedHandler: ((event: { payload: { count: number; latestFilename: string | null } }) => void) | undefined;

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: listenMock,
}));

function Harness() {
  const navigation = usePreviewNavigation({
    imagePath: '/photos/current.jpg',
    onImagePathChange: () => {},
    onBeforeNavigation: () => {},
    onNavigationSettled: () => {},
  });

  return (
    <div>
      <span data-testid="index">{navigation.currentIndex}</span>
      <span data-testid="total">{navigation.totalFiles}</span>
      <button data-testid="oldest" onClick={navigation.goToOldest}>oldest</button>
    </div>
  );
}

describe('usePreviewNavigation', () => {
  const { getContainer, getRoot } = setupReactRoot();

  beforeEach(() => {
    fileIndexChangedHandler = undefined;
    invokeMock.mockReset();
    listenMock.mockReset();

    listenMock.mockImplementation(async (name: string, handler: (event: { payload: { count: number; latestFilename: string | null } }) => void) => {
      if (name === 'file-index-changed') {
        fileIndexChangedHandler = handler;
      }
      return vi.fn();
    });

    invokeMock.mockImplementation(async (command: string, args?: { index: number }) => {
      if (command === 'get_file_list') {
        return [
          { path: '/photos/0.jpg' },
          { path: '/photos/1.jpg' },
          { path: '/photos/2.jpg' },
        ];
      }

      if (command === 'get_current_file_index') {
        return 2;
      }

      if (command === 'navigate_to_file') {
        return { path: `/photos/${args?.index ?? 0}.jpg` };
      }

      return null;
    });
  });

  it('loads initial file info and clamps index on file-index-changed', async () => {
    await act(async () => {
      getRoot().render(<Harness />);
      await flush();
    });

    expect(getContainer().querySelector('[data-testid="total"]')?.textContent).toBe('3');
    expect(getContainer().querySelector('[data-testid="index"]')?.textContent).toBe('2');

    await act(async () => {
      fileIndexChangedHandler?.({ payload: { count: 1, latestFilename: null } });
      await flush();
    });

    expect(getContainer().querySelector('[data-testid="total"]')?.textContent).toBe('1');
    expect(getContainer().querySelector('[data-testid="index"]')?.textContent).toBe('0');
  });

  it('navigates to oldest file', async () => {
    const onImagePathChange = vi.fn();

    function NavigateHarness() {
      const navigation = usePreviewNavigation({
        imagePath: '/photos/current.jpg',
        onImagePathChange,
        onBeforeNavigation: () => {},
        onNavigationSettled: () => {},
      });

      return <button data-testid="oldest" onClick={navigation.goToOldest}>oldest</button>;
    }

    await act(async () => {
      getRoot().render(<NavigateHarness />);
      await flush();
    });

    await act(async () => {
      getContainer().querySelector('[data-testid="oldest"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    expect(invokeMock).toHaveBeenCalledWith('navigate_to_file', { index: 2 });
    expect(onImagePathChange).toHaveBeenCalledWith('/photos/2.jpg');
  });
});
