/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { act } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { PREVIEW_NAVIGATE_EVENT } from '../preview-window-events';
import { usePreviewWindowLifecycle } from '../usePreviewWindowLifecycle';
import { flush } from '../../test-utils/flush';
import { setupReactRoot } from '../../test-utils/react-root';

const { invokeMock, listenMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  listenMock: vi.fn(),
}));

type PreviewPayload = { file_path: string; bring_to_front: boolean };

let previewHandler: ((event: { payload: PreviewPayload }) => void) | undefined;

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: listenMock,
}));

function Harness() {
  const state = usePreviewWindowLifecycle();
  return (
    <div>
      <span data-testid="is-open">{state.isOpen ? 'yes' : 'no'}</span>
      <span data-testid="image">{state.currentImage ?? ''}</span>
      <span data-testid="bring-front">{state.autoBringToFront ? 'yes' : 'no'}</span>
    </div>
  );
}

describe('usePreviewWindowLifecycle', () => {
  const { getContainer, getRoot } = setupReactRoot();

  beforeEach(() => {
    previewHandler = undefined;
    invokeMock.mockReset();
    listenMock.mockReset();

    invokeMock.mockResolvedValue('windows');
    listenMock.mockImplementation(async (name: string, handler: (event: { payload: PreviewPayload }) => void) => {
      if (name === 'preview-image') {
        previewHandler = handler;
      }
      return vi.fn();
    });
  });

  it('loads platform class and handles preview and navigate events', async () => {
    await act(async () => {
      getRoot().render(<Harness />);
      await flush();
    });

    expect(document.documentElement.className).toBe('platform-windows');

    await act(async () => {
      previewHandler?.({ payload: { file_path: '/photos/a.jpg', bring_to_front: true } });
      await flush();
    });

    expect(getContainer().querySelector('[data-testid="is-open"]')?.textContent).toBe('yes');
    expect(getContainer().querySelector('[data-testid="image"]')?.textContent).toBe('/photos/a.jpg');
    expect(getContainer().querySelector('[data-testid="bring-front"]')?.textContent).toBe('yes');

    await act(async () => {
      window.dispatchEvent(new CustomEvent(PREVIEW_NAVIGATE_EVENT, { detail: '/photos/b.jpg' }));
      await flush();
    });

    expect(getContainer().querySelector('[data-testid="image"]')?.textContent).toBe('/photos/b.jpg');
  });
});
