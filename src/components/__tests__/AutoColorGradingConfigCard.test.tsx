/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { act } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { flush } from '../../test-utils/flush';
import { setupReactRoot } from '../../test-utils/react-root';

const { updateDraftMock, invokeMock, setDraft, useDraftConfigMock } = vi.hoisted(() => {
  let draftState: Record<string, unknown> | null = null;
  return {
    updateDraftMock: vi.fn().mockImplementation((fn: (d: Record<string, unknown>) => Record<string, unknown>) => {
      if (draftState) draftState = fn(draftState);
    }),
    invokeMock: vi.fn().mockResolvedValue([
      { id: 'fujifilm-provia', displayName: 'Fuji Provia' },
      { id: 'fujifilm-velvia', displayName: 'Fuji Velvia' },
    ]),
    useDraftConfigMock: () => draftState,
    setDraft: (d: Record<string, unknown> | null) => { draftState = d; },
  };
});

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

vi.mock('../../stores/configStore', () => {
  const state = () => ({
    updateDraft: updateDraftMock,
    isLoading: false,
  });
  return {
    useConfigStore: (selector?: (s: ReturnType<typeof state>) => unknown) =>
      selector ? selector(state()) : state(),
    useDraftConfig: useDraftConfigMock,
  };
});

import { AutoColorGradingConfigCard } from '../AutoColorGradingConfigCard';

describe('AutoColorGradingConfigCard', () => {
  const { getContainer, getRoot } = setupReactRoot();

  function renderWithDraft(draft: Record<string, unknown> | null) {
    setDraft(draft);
    act(() => {
      getRoot().render(<AutoColorGradingConfigCard />);
    });
  }

  beforeEach(() => {
    setDraft(null);
    updateDraftMock.mockClear();
    invokeMock.mockClear();
    invokeMock.mockResolvedValue([
      { id: 'fujifilm-provia', displayName: 'Fuji Provia' },
      { id: 'fujifilm-velvia', displayName: 'Fuji Velvia' },
    ]);
  });

  it('returns null when autoColorGrading config is missing', () => {
    renderWithDraft(null);
    expect(getContainer().innerHTML).toBe('');
  });

  it('flips option visibility when the enable toggle is clicked', async () => {
    renderWithDraft({
      autoColorGrading: {
        enabled: false,
        presetId: 'fujifilm-provia',
        meteringMode: 'matrix',
        evOffset: 0,
      },
    });
    await act(async () => { await flush(); });
    await act(async () => { await flush(); });

    const toggle = getContainer().querySelector('button[aria-label="自动调色"]') as HTMLButtonElement;
    expect(toggle.getAttribute('aria-pressed')).toBe('false');
    expect(getContainer().textContent).not.toContain('调色预设');

    // Click flips enabled in the draft via updateDraft; remount to observe the
    // conditional section. （组件是 memo 且 mock store 不建立订阅，对同组件的
    //   重复 render 会被 memo 跳过，必须先卸载再挂载才能读到新 draft。）
    await act(async () => {
      toggle.click();
      await flush();
    });
    act(() => {
      getRoot().render(null);
    });
    act(() => {
      getRoot().render(<AutoColorGradingConfigCard />);
    });
    await act(async () => { await flush(); });

    const enabledToggle = getContainer().querySelector('button[aria-label="自动调色"]') as HTMLButtonElement;
    expect(enabledToggle.getAttribute('aria-pressed')).toBe('true');
    const text = getContainer().textContent ?? '';
    expect(text).toContain('调色预设');
    expect(text).toContain('曝光偏移');
    expect(text).toContain('测光模式');

    // Clicking again hides the options once more.（同样需要卸载重挂载。）
    await act(async () => {
      enabledToggle.click();
      await flush();
    });
    act(() => {
      getRoot().render(null);
    });
    act(() => {
      getRoot().render(<AutoColorGradingConfigCard />);
    });
    await act(async () => { await flush(); });

    expect(getContainer().textContent).not.toContain('调色预设');
  });

  it('toggles enabled state on toggle click', async () => {
    renderWithDraft({
      autoColorGrading: {
        enabled: false,
        presetId: 'fujifilm-provia',
        meteringMode: 'matrix',
        evOffset: 0,
      },
    });
    await act(async () => { await flush(); });
    await act(async () => { await flush(); });

    const toggle = getContainer().querySelector('button[aria-label="自动调色"]');
    await act(async () => {
      (toggle as HTMLElement)!.click();
      await flush();
    });

    expect(updateDraftMock).toHaveBeenCalled();
  });
});
