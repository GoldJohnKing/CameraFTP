/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { act } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { TaskProgressPanel } from '../TaskProgressPanel';
import { setupReactRoot } from '../../test-utils/react-root';

const {
  mockAiEditState,
  mockCgState,
  cancelAiEditMock,
  cancelColorGradingMock,
  dismissAiEditDoneMock,
  dismissColorGradingDoneMock,
} = vi.hoisted(() => {
  const ai = {
    isEditing: false,
    isDone: false,
    current: 0,
    total: 0,
    currentFileName: '',
    failedCount: 0,
    failedFiles: [] as string[],
  };
  const cg = {
    isProcessing: false,
    isDone: false,
    current: 0,
    total: 0,
    currentFileName: '',
    failedCount: 0,
    failedFiles: [] as string[],
  };
  return {
    mockAiEditState: ai,
    mockCgState: cg,
    cancelAiEditMock: vi.fn().mockResolvedValue(undefined),
    cancelColorGradingMock: vi.fn().mockResolvedValue(undefined),
    dismissAiEditDoneMock: vi.fn(),
    dismissColorGradingDoneMock: vi.fn(),
  };
});

vi.mock('../../hooks/useAiEditProgress', () => ({
  useAiEditProgress: () => mockAiEditState,
}));

vi.mock('../../hooks/useColorGradingProgress', () => ({
  useColorGradingProgress: () => mockCgState,
}));

vi.mock('../../services/ai-edit-tasks', () => ({
  cancelAiEdit: cancelAiEditMock,
  dismissDone: dismissAiEditDoneMock,
}));

vi.mock('../../services/color-grading-tasks', () => ({
  cancelColorGrading: cancelColorGradingMock,
  dismissColorGradingDone: dismissColorGradingDoneMock,
}));

function resetMocks() {
  Object.assign(mockAiEditState, {
    isEditing: false,
    isDone: false,
    current: 0,
    total: 0,
    currentFileName: '',
    failedCount: 0,
    failedFiles: [],
  });
  Object.assign(mockCgState, {
    isProcessing: false,
    isDone: false,
    current: 0,
    total: 0,
    currentFileName: '',
    failedCount: 0,
    failedFiles: [],
  });
  cancelAiEditMock.mockClear();
  cancelColorGradingMock.mockClear();
  dismissAiEditDoneMock.mockClear();
  dismissColorGradingDoneMock.mockClear();
}

describe('TaskProgressPanel', () => {
  const { getContainer, getRoot } = setupReactRoot();

  beforeEach(() => {
    resetMocks();
  });

  function render(position: 'absolute' | 'fixed' = 'absolute') {
    act(() => {
      getRoot().render(<TaskProgressPanel position={position} />);
    });
  }

  it('returns null when no tasks are active', () => {
    render();
    expect(getContainer().innerHTML).toBe('');
  });

  it('shows AI edit row when editing', () => {
    Object.assign(mockAiEditState, {
      isEditing: true,
      current: 2,
      total: 4,
    });
    render();
    const html = getContainer().textContent ?? '';
    expect(html).toContain('AI修图');
    expect(html).toContain('2 / 4');
    expect(html).toContain('全部取消');
  });

  it('shows color grading row when processing', () => {
    Object.assign(mockCgState, {
      isProcessing: true,
      current: 1,
      total: 3,
    });
    render();
    const html = getContainer().textContent ?? '';
    expect(html).toContain('调色');
    expect(html).toContain('1 / 3');
  });

  it('shows both rows when both active', () => {
    Object.assign(mockAiEditState, { isEditing: true, current: 1, total: 2 });
    Object.assign(mockCgState, { isProcessing: true, current: 3, total: 5 });
    render();
    const html = getContainer().textContent ?? '';
    expect(html).toContain('AI修图');
    expect(html).toContain('调色');
    expect(html).toContain('1 / 2');
    expect(html).toContain('3 / 5');
  });

  it('shows failure count when failures > 0', () => {
    Object.assign(mockAiEditState, {
      isEditing: true,
      current: 2,
      total: 4,
      failedCount: 1,
    });
    render();
    const html = getContainer().textContent ?? '';
    expect(html).toContain('失败 1');
  });

  it('shows "已完成" when all done', () => {
    Object.assign(mockAiEditState, { isDone: true, total: 2, current: 2 });
    render();
    const html = getContainer().textContent ?? '';
    expect(html).toContain('已完成');
    expect(html).not.toContain('全部取消');
  });

  it('calls cancelAiEdit when × clicked on AI edit row', () => {
    Object.assign(mockAiEditState, { isEditing: true, current: 1, total: 2 });
    render();
    const btn = getContainer().querySelector('button[aria-label="取消AI修图"]');
    expect(btn).toBeTruthy();
    act(() => {
      btn!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    expect(cancelAiEditMock).toHaveBeenCalledTimes(1);
  });

  it('calls both cancels when "全部取消" clicked', () => {
    Object.assign(mockAiEditState, { isEditing: true, current: 1, total: 2 });
    Object.assign(mockCgState, { isProcessing: true, current: 1, total: 2 });
    render();
    const footer = getContainer().querySelector('.border-t button');
    expect(footer).toBeTruthy();
    act(() => {
      footer!.dispatchEvent(new MouseEvent('click', { bubbles: true }));
    });
    expect(cancelAiEditMock).toHaveBeenCalledTimes(1);
    expect(cancelColorGradingMock).toHaveBeenCalledTimes(1);
  });

  it('hides × button on done rows', () => {
    Object.assign(mockAiEditState, { isDone: true, total: 2, current: 2 });
    render();
    const cancelBtn = getContainer().querySelector('button[aria-label="取消AI修图"]');
    expect(cancelBtn).toBeNull();
  });

  it('applies fixed positioning when position="fixed"', () => {
    Object.assign(mockAiEditState, { isEditing: true, current: 1, total: 2 });
    render('fixed');
    const outer = getContainer().firstElementChild as HTMLElement;
    expect(outer.className).toContain('fixed');
    expect(outer.style.bottom).toBe('5rem');
  });

  it('applies absolute positioning when position="absolute"', () => {
    Object.assign(mockAiEditState, { isEditing: true, current: 1, total: 2 });
    render('absolute');
    const outer = getContainer().firstElementChild as HTMLElement;
    expect(outer.className).toContain('absolute');
    expect(outer.style.bottom).toBe('76px');
  });

  it('auto-dismisses after 3 seconds when all done', () => {
    vi.useFakeTimers();
    Object.assign(mockAiEditState, { isDone: true, total: 2, current: 2 });

    act(() => {
      getRoot().render(<TaskProgressPanel position="absolute" />);
    });

    expect(dismissAiEditDoneMock).not.toHaveBeenCalled();
    expect(dismissColorGradingDoneMock).not.toHaveBeenCalled();

    act(() => {
      vi.advanceTimersByTime(3000);
    });

    expect(dismissAiEditDoneMock).toHaveBeenCalledTimes(1);
    expect(dismissColorGradingDoneMock).toHaveBeenCalledTimes(1);

    vi.useRealTimers();
  });
});
