/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { act } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { flush } from '../../test-utils/flush';
import { setupReactRoot } from '../../test-utils/react-root';
import type { ColorGradingPreset } from '../../types';

const { updateDraftMock, setDraft, useDraftConfigMock, toastErrorMock } = vi.hoisted(() => {
  let draftState: Record<string, unknown> | null = null;
  return {
    updateDraftMock: vi.fn().mockImplementation((fn: (d: Record<string, unknown>) => Record<string, unknown>) => {
      if (draftState) draftState = fn(draftState);
    }),
    useDraftConfigMock: () => draftState,
    setDraft: (d: Record<string, unknown> | null) => { draftState = d; },
    toastErrorMock: vi.fn(),
  };
});

vi.mock('sonner', () => ({
  toast: { error: toastErrorMock },
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

import { ColorGradingDialog } from '../ColorGradingDialog';

const PRESETS: ColorGradingPreset[] = [
  { id: 'fujifilm-provia', displayName: 'Fuji Provia', logSpace: 'V-Log', cubeFilename: 'Fujifilm_PROVIA_VLog.cube' },
  { id: 'fujifilm-velvia', displayName: 'Fuji Velvia', logSpace: 'V-Log', cubeFilename: 'Fujifilm_Velvia_VLog.cube' },
  { id: 'kodak-portra', displayName: 'Kodak Portra', logSpace: 'V-Log', cubeFilename: 'Kodak_Portra_VLog.cube' },
];

describe('ColorGradingDialog', () => {
  const { getContainer, getRoot } = setupReactRoot();
  const onConfirm = vi.fn();
  const onCancel = vi.fn();

  beforeEach(() => {
    onConfirm.mockClear();
    onCancel.mockClear();
    updateDraftMock.mockClear();
    toastErrorMock.mockClear();
    setDraft({
      colorGradingLastUsed: null,
      autoColorGrading: null,
    });
  });

  function renderDialog(isOpen = true) {
    act(() => {
      getRoot().render(
        <ColorGradingDialog
          isOpen={isOpen}
          colorGradingPresets={PRESETS}
          onConfirm={onConfirm}
          onCancel={onCancel}
        />,
      );
    });
  }

  it('renders nothing when not open', () => {
    renderDialog(false);
    expect(getContainer().innerHTML).toBe('');
  });

  it('renders preset selector when open', async () => {
    renderDialog(true);
    await act(async () => { await flush(); });

    expect(getContainer().textContent).toContain('调色预设');
    expect(getContainer().textContent).toContain('调色');
  });

  it('always shows metering mode and EV offset slider', async () => {
    renderDialog(true);
    await act(async () => { await flush(); });

    expect(getContainer().textContent).toContain('测光模式');
    expect(getContainer().textContent).toContain('曝光偏移');
  });

  it('calls onConfirm with default params when apply is clicked', async () => {
    renderDialog(true);
    await act(async () => { await flush(); });

    const applyButton = Array.from(getContainer().querySelectorAll('button')).find(
      b => b.textContent === '应用',
    );
    expect(applyButton).toBeTruthy();

    await act(async () => {
      applyButton!.click();
      await flush();
    });

    expect(onConfirm).toHaveBeenCalledWith('fujifilm-provia', 'matrix', 0);
    expect(updateDraftMock).toHaveBeenCalled();
    expect(toastErrorMock).not.toHaveBeenCalled();
  });

  it('toasts error instead of throwing when onConfirm rejects', async () => {
    onConfirm.mockRejectedValue(new Error('enqueue failed'));

    renderDialog(true);
    await act(async () => { await flush(); });

    const applyButton = Array.from(getContainer().querySelectorAll('button')).find(
      b => b.textContent === '应用',
    );
    expect(applyButton).toBeTruthy();

    await act(async () => {
      applyButton!.click();
      await flush();
    });

    expect(onConfirm).toHaveBeenCalledWith('fujifilm-provia', 'matrix', 0);
    expect(toastErrorMock).toHaveBeenCalledWith('调色失败：enqueue failed');
  });

  it('calls onCancel when cancel button is clicked', async () => {
    renderDialog(true);
    await act(async () => { await flush(); });

    const cancelButton = Array.from(getContainer().querySelectorAll('button')).find(
      b => b.textContent === '取消',
    );
    expect(cancelButton).toBeTruthy();

    await act(async () => {
      cancelButton!.click();
      await flush();
    });

    expect(onCancel).toHaveBeenCalled();
  });
});
