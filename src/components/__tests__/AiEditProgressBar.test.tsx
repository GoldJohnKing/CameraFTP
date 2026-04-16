/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { act } from 'react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { flush } from '../../test-utils/flush';
import { setupReactRoot } from '../../test-utils/react-root';

// Mock state shared between mock factory and test code
const {
  mockState,
  dismissDoneMock,
  cancelAiEditMock,
} = vi.hoisted(() => {
  const state = {
    isEditing: false,
    isDone: false,
    current: 0,
    total: 0,
    failedCount: 0,
  };
  return {
    mockState: state,
    dismissDoneMock: vi.fn(),
    cancelAiEditMock: vi.fn().mockResolvedValue(undefined),
  };
});

vi.mock('../../hooks/useAiEditProgress', () => {
  const createHook = () => mockState;
  return {
    useAiEditProgress: createHook,
    dismissDone: dismissDoneMock,
    cancelAiEdit: cancelAiEditMock,
  };
});

import { AiEditProgressBar } from '../AiEditProgressBar';

describe('AiEditProgressBar', () => {
  const { getContainer, getRoot } = setupReactRoot();

  beforeEach(() => {
    mockState.isEditing = false;
    mockState.isDone = false;
    mockState.current = 0;
    mockState.total = 0;
    mockState.failedCount = 0;
    dismissDoneMock.mockClear();
    cancelAiEditMock.mockClear();
  });

  it('returns null when not editing and not done', async () => {
    await act(async () => {
      getRoot().render(<AiEditProgressBar position="fixed" />);
      await flush();
    });

    expect(getContainer().innerHTML).toBe('');
  });

  it('shows cancel button text during editing', async () => {
    mockState.isEditing = true;
    mockState.current = 1;
    mockState.total = 2;

    await act(async () => {
      getRoot().render(<AiEditProgressBar position="fixed" />);
      await flush();
    });

    const button = getContainer().querySelector('button');
    expect(button).toBeTruthy();
    expect(button?.textContent).toBe('取消');
  });

  it('shows X icon when done with failures', async () => {
    mockState.isDone = true;
    mockState.failedCount = 1;

    await act(async () => {
      getRoot().render(<AiEditProgressBar position="fixed" />);
      await flush();
    });

    const button = getContainer().querySelector('button');
    expect(button).toBeTruthy();
    // X icon is an SVG, no text content for the icon
    expect(button?.textContent).not.toBe('取消');
  });

  it('calls cancelAiEdit when cancel button clicked during editing', async () => {
    mockState.isEditing = true;
    mockState.current = 1;
    mockState.total = 2;

    await act(async () => {
      getRoot().render(<AiEditProgressBar position="fixed" />);
      await flush();
    });

    const button = getContainer().querySelector('button');
    await act(async () => {
      button?.click();
      await flush();
    });

    expect(cancelAiEditMock).toHaveBeenCalled();
  });

  it('calls dismissDone when X button clicked when done', async () => {
    mockState.isDone = true;
    mockState.failedCount = 1;

    await act(async () => {
      getRoot().render(<AiEditProgressBar position="fixed" />);
      await flush();
    });

    const button = getContainer().querySelector('button');
    await act(async () => {
      button?.click();
      await flush();
    });

    expect(dismissDoneMock).toHaveBeenCalled();
  });

  it('positions fixed progress bar above bottom nav (bottom-20)', async () => {
    mockState.isEditing = true;
    mockState.current = 1;
    mockState.total = 1;

    await act(async () => {
      getRoot().render(<AiEditProgressBar position="fixed" />);
      await flush();
    });

    const bar = getContainer().firstElementChild as HTMLElement;
    expect(bar).toBeTruthy();
    expect(bar.className).toContain('bottom-20');
    expect(bar.className).toContain('fixed');
    expect(bar.className).toContain('z-50');
  });

  it('positions absolute progress bar above toolbar', async () => {
    mockState.isEditing = true;
    mockState.current = 1;
    mockState.total = 1;

    await act(async () => {
      getRoot().render(<AiEditProgressBar position="absolute" />);
      await flush();
    });

    const bar = getContainer().firstElementChild as HTMLElement;
    expect(bar).toBeTruthy();
    expect(bar.className).toContain('absolute');
    expect(bar.style.bottom).toBe('76px');
  });
});
