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

vi.mock('../../stores/configStore', () => ({
  useConfigStore: () => ({
    updateDraft: updateDraftMock,
    isLoading: false,
  }),
  useDraftConfig: useDraftConfigMock,
}));

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

  it('renders enable/disable toggle', async () => {
    renderWithDraft({
      autoColorGrading: {
        enabled: false,
        presetId: 'fujifilm-provia',
        meteringMode: 'highlight-safe',
        evOffset: 0,
      },
    });
    await act(async () => { await flush(); });
    await act(async () => { await flush(); });

    expect(getContainer().textContent).toContain('自动调色');
    const toggle = getContainer().querySelector('button[aria-label="自动调色"]');
    expect(toggle).toBeTruthy();
    expect(toggle!.getAttribute('aria-pressed')).toBe('false');
  });

  it('shows config options when enabled', async () => {
    renderWithDraft({
      autoColorGrading: {
        enabled: true,
        presetId: 'fujifilm-provia',
        meteringMode: 'highlight-safe',
        evOffset: 0,
      },
    });
    await act(async () => { await flush(); });
    await act(async () => { await flush(); });

    expect(getContainer().textContent).toContain('调色预设');
    expect(getContainer().textContent).toContain('曝光偏移');
    expect(getContainer().textContent).toContain('测光模式');
  });

  it('hides config options when disabled', async () => {
    renderWithDraft({
      autoColorGrading: {
        enabled: false,
        presetId: 'fujifilm-provia',
        meteringMode: 'highlight-safe',
        evOffset: 0,
      },
    });
    await act(async () => { await flush(); });
    await act(async () => { await flush(); });

    expect(getContainer().textContent).not.toContain('调色预设');
  });

  it('toggles enabled state on toggle click', async () => {
    renderWithDraft({
      autoColorGrading: {
        enabled: false,
        presetId: 'fujifilm-provia',
        meteringMode: 'highlight-safe',
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
