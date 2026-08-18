/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { act } from 'react';
import { fireEvent, within } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { flush } from '../../test-utils/flush';
import { setupReactRoot } from '../../test-utils/react-root';

const lifecycleState = {
  isOpen: true,
  currentImage: '/tmp/example.jpg',
  autoBringToFront: false,
};

const {
  invokeMock,
  listenMock,
  updatePreviewConfigMock,
  goToPreviousMock,
  goToNextMock,
  goToOldestMock,
  goToLatestMock,
  isFullscreenMock,
  setFullscreenMock,
  setAlwaysOnTopMock,
} = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  listenMock: vi.fn(),
  updatePreviewConfigMock: vi.fn().mockResolvedValue({ autoBringToFront: false }),
  goToPreviousMock: vi.fn(),
  goToNextMock: vi.fn(),
  goToOldestMock: vi.fn(),
  goToLatestMock: vi.fn(),
  isFullscreenMock: vi.fn(),
  setFullscreenMock: vi.fn(),
  setAlwaysOnTopMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  convertFileSrc: (path: string) => path,
  invoke: invokeMock,
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: listenMock,
}));

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({
    isFullscreen: isFullscreenMock,
    onResized: vi.fn().mockResolvedValue(() => {}),
    setFullscreen: setFullscreenMock,
    setAlwaysOnTop: setAlwaysOnTopMock,
  }),
}));

vi.mock('../../stores/configStore', () => ({
  useConfigStore: (selector?: (state: { updatePreviewConfig: typeof updatePreviewConfigMock }) => unknown) =>
    selector ? selector({ updatePreviewConfig: updatePreviewConfigMock }) : { updatePreviewConfig: updatePreviewConfigMock },
  useDraftConfig: () => null,
}));

vi.mock('../../hooks/usePreviewWindowLifecycle', () => ({
  usePreviewWindowLifecycle: () => lifecycleState,
}));

vi.mock('../../hooks/usePreviewNavigation', () => ({
  usePreviewNavigation: () => ({
    currentIndex: 1,
    totalFiles: 3,
    goToPrevious: goToPreviousMock,
    goToNext: goToNextMock,
    goToOldest: goToOldestMock,
    goToLatest: goToLatestMock,
  }),
}));

vi.mock('../../hooks/usePreviewExif', () => ({
  usePreviewExif: () => null,
}));

vi.mock('../../hooks/usePreviewZoomPan', () => ({
  usePreviewZoomPan: () => ({
    scale: 1,
    panX: 0,
    panY: 0,
    isDragging: false,
    containerRef: { current: null },
    resetZoom: vi.fn(),
    handleWheel: vi.fn(),
    handleMouseDown: vi.fn(),
    handleMouseMove: vi.fn(),
    stopDragging: vi.fn(),
  }),
}));

vi.mock('../../hooks/usePreviewToolbarAutoHide', () => ({
  usePreviewToolbarAutoHide: () => ({
    showToolbar: true,
    showToolbarOnPointerMove: vi.fn(),
    handleToolbarMouseEnter: vi.fn(),
    handleToolbarMouseLeave: vi.fn(),
  }),
}));

vi.mock('../../hooks/useColorGradingPresets', () => ({
  useColorGradingPresets: () => [
    { id: 'fujifilm-provia', displayName: 'Fuji Provia', logSpace: 'V-Log', cubeFilename: 'Fujifilm_PROVIA_VLog.cube' },
  ],
}));

import { PreviewWindow } from '../PreviewWindow';

describe('PreviewWindow keyboard navigation', () => {
  const { getContainer, getRoot } = setupReactRoot();

  const renderWindow = async () => {
    await act(async () => {
      getRoot().render(<PreviewWindow />);
      await flush();
    });
  };

  const pressKey = async (key: string) => {
    await act(async () => {
      fireEvent.keyDown(window, { key });
      await flush();
    });
  };

  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(undefined);
    listenMock.mockReset();
    listenMock.mockResolvedValue(() => {});
    updatePreviewConfigMock.mockClear();
    goToPreviousMock.mockClear();
    goToNextMock.mockClear();
    goToOldestMock.mockClear();
    goToLatestMock.mockClear();
    setFullscreenMock.mockClear();
    setAlwaysOnTopMock.mockClear();
    isFullscreenMock.mockReset();
    isFullscreenMock.mockResolvedValue(false);
    lifecycleState.isOpen = true;
    lifecycleState.currentImage = '/tmp/example.jpg';
    lifecycleState.autoBringToFront = false;
  });

  it('navigates to the previous image on ArrowLeft and ArrowUp', async () => {
    await renderWindow();

    await pressKey('ArrowLeft');
    expect(goToPreviousMock).toHaveBeenCalledTimes(1);

    await pressKey('ArrowUp');
    expect(goToPreviousMock).toHaveBeenCalledTimes(2);
    expect(goToNextMock).not.toHaveBeenCalled();
  });

  it('navigates to the next image on ArrowRight and ArrowDown', async () => {
    await renderWindow();

    await pressKey('ArrowRight');
    expect(goToNextMock).toHaveBeenCalledTimes(1);

    await pressKey('ArrowDown');
    expect(goToNextMock).toHaveBeenCalledTimes(2);
    expect(goToPreviousMock).not.toHaveBeenCalled();
  });

  it('jumps to the oldest image on Home and the latest on End', async () => {
    await renderWindow();

    await pressKey('Home');
    expect(goToOldestMock).toHaveBeenCalledTimes(1);
    expect(goToLatestMock).not.toHaveBeenCalled();

    await pressKey('End');
    expect(goToLatestMock).toHaveBeenCalledTimes(1);
  });

  it('exits fullscreen on Escape while fullscreen', async () => {
    isFullscreenMock.mockResolvedValue(true);
    await renderWindow();

    await pressKey('Escape');

    expect(setFullscreenMock).toHaveBeenCalledWith(false);
    expect(setAlwaysOnTopMock).toHaveBeenCalledWith(false);
    expect(goToPreviousMock).not.toHaveBeenCalled();
  });

  it('suppresses navigation while the AI edit dialog is open and Escape closes it', async () => {
    await renderWindow();

    const aiButton = getContainer().querySelector('button[title="AI修图"]') as HTMLButtonElement;
    expect(aiButton).toBeTruthy();
    await act(async () => {
      aiButton.click();
      await flush();
    });

    // Dialog is open — its title is visible.
    expect(within(getContainer()).getByText('AI 修图')).toBeTruthy();

    await pressKey('ArrowLeft');
    await pressKey('Home');
    expect(goToPreviousMock).not.toHaveBeenCalled();
    expect(goToOldestMock).not.toHaveBeenCalled();

    // Escape closes the dialog instead of touching the window.
    await pressKey('Escape');
    expect(within(getContainer()).queryByText('AI 修图')).toBeNull();

    // Navigation resumes once the dialog is closed.
    await pressKey('ArrowLeft');
    expect(goToPreviousMock).toHaveBeenCalledTimes(1);
  });

  it('Escape closes the color grading dialog before exiting fullscreen', async () => {
    isFullscreenMock.mockResolvedValue(true);
    lifecycleState.currentImage = '/tmp/example.dng';
    await renderWindow();

    const colorButton = getContainer().querySelector('button[title="调色"]') as HTMLButtonElement;
    expect(colorButton).toBeTruthy();
    expect(colorButton.disabled).toBe(false);

    await act(async () => {
      colorButton.click();
      await flush();
    });
    expect(within(getContainer()).getByText('调色')).toBeTruthy();

    // First Escape only closes the dialog; fullscreen must be untouched.
    await pressKey('Escape');
    expect(within(getContainer()).queryByText('调色')).toBeNull();
    expect(setFullscreenMock).not.toHaveBeenCalled();

    // Second Escape (no dialog open) exits fullscreen.
    await pressKey('Escape');
    expect(setFullscreenMock).toHaveBeenCalledWith(false);
    expect(setAlwaysOnTopMock).toHaveBeenCalledWith(false);
  });
});
