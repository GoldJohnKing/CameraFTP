/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useQuitFlow } from '../useQuitFlow';

const { invokeMock, listenMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  listenMock: vi.fn(),
}));

const eventHandlers = new Map<string, () => void | Promise<void>>();

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: listenMock,
}));

function QuitFlowHarness({ enabled = true }: { enabled?: boolean }) {
  const { showQuitDialog, closeQuitDialog, handleQuitConfirm } = useQuitFlow({ enabled });

  return (
    <div>
      <span data-testid="visible">{showQuitDialog ? 'yes' : 'no'}</span>
      <button onClick={closeQuitDialog} data-testid="close">close</button>
      <button onClick={() => handleQuitConfirm(false)} data-testid="minimize">minimize</button>
      <button onClick={() => handleQuitConfirm(true)} data-testid="quit">quit</button>
    </div>
  );
}

async function flush(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}

describe('useQuitFlow', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);

    eventHandlers.clear();
    invokeMock.mockReset();
    listenMock.mockReset();
    listenMock.mockImplementation(async (name: string, handler: () => void | Promise<void>) => {
      eventHandlers.set(name, handler);
      return vi.fn();
    });
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    eventHandlers.clear();
    vi.unstubAllGlobals();
  });

  it('opens quit dialog after window-close-requested and requests window show', async () => {
    await act(async () => {
      root.render(<QuitFlowHarness />);
      await flush();
    });

    await act(async () => {
      await eventHandlers.get('window-close-requested')?.();
      await flush();
    });

    expect(invokeMock).toHaveBeenCalledWith('show_main_window');
    expect(container.querySelector('[data-testid="visible"]')?.textContent).toBe('yes');
  });

  it('quits application when confirmed', async () => {
    await act(async () => {
      root.render(<QuitFlowHarness />);
      await flush();
    });

    await act(async () => {
      container.querySelector('[data-testid="quit"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    expect(invokeMock).toHaveBeenCalledWith('quit_application');
  });

  it('hides main window and closes dialog when minimizing', async () => {
    await act(async () => {
      root.render(<QuitFlowHarness />);
      await flush();
    });

    await act(async () => {
      await eventHandlers.get('window-close-requested')?.();
      await flush();
    });

    await act(async () => {
      container.querySelector('[data-testid="minimize"]')?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
      await flush();
    });

    expect(invokeMock).toHaveBeenCalledWith('hide_main_window');
    expect(container.querySelector('[data-testid="visible"]')?.textContent).toBe('no');
  });

  it('skips event subscription when quit flow is disabled', async () => {
    await act(async () => {
      root.render(<QuitFlowHarness enabled={false} />);
      await flush();
    });

    expect(listenMock).not.toHaveBeenCalled();
  });

  it('cleans up listener when async setup resolves after unmount', async () => {
    let resolveListen: ((unlisten: () => void) => void) | undefined;
    const unlistenMock = vi.fn();

    listenMock.mockImplementationOnce(
      () => new Promise((resolve) => {
        resolveListen = resolve;
      }),
    );

    await act(async () => {
      root.render(<QuitFlowHarness />);
      await flush();
    });

    act(() => {
      root.unmount();
    });

    await act(async () => {
      resolveListen?.(unlistenMock);
      await flush();
    });

    expect(unlistenMock).toHaveBeenCalledTimes(1);
  });
});
