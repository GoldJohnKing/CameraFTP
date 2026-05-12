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

const { updateDraftMock, setDraft, useDraftConfigMock } = vi.hoisted(() => {
  let draftState: Record<string, unknown> | null = null;
  return {
    updateDraftMock: vi.fn().mockImplementation((fn: (d: Record<string, unknown>) => Record<string, unknown>) => {
      if (draftState) draftState = fn(draftState);
    }),
    useDraftConfigMock: () => draftState,
    setDraft: (d: Record<string, unknown> | null) => { draftState = d; },
  };
});

vi.mock('../../stores/configStore', () => ({
  useConfigStore: () => ({
    updateDraft: updateDraftMock,
    isLoading: false,
  }),
  useDraftConfig: useDraftConfigMock,
}));

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

  it('toggles auto/manual exposure', async () => {
    renderDialog(true);
    await act(async () => { await flush(); });

    expect(getContainer().textContent).toContain('自动曝光');
    expect(getContainer().textContent).toContain('测光模式');

    const toggles = getContainer().querySelectorAll('button[aria-pressed]');
    const autoExposureToggle = Array.from(toggles).find(b => b.getAttribute('aria-label') === '自动曝光');
    expect(autoExposureToggle).toBeTruthy();

    await act(async () => {
      (autoExposureToggle as HTMLElement)!.click();
      await flush();
    });

    expect(getContainer().textContent).toContain('曝光补偿');
    expect(getContainer().textContent).not.toContain('测光模式');
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

    expect(onConfirm).toHaveBeenCalledWith('fujifilm-provia', true, 'highlight-safe', 0);
    expect(updateDraftMock).toHaveBeenCalled();
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
