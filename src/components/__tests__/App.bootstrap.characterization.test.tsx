/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import App from '../../App';

const {
  initializeServerEventsMock,
  handleQuitConfirmMock,
  closeQuitDialogMock,
  closePermissionDialogMock,
  continueAfterPermissionsGrantedMock,
  loadPlatformMock,
  loadConfigMock,
  initializePermissionsMock,
  useQuitFlowMock,
} = vi.hoisted(() => ({
  initializeServerEventsMock: vi.fn(),
  handleQuitConfirmMock: vi.fn(),
  closeQuitDialogMock: vi.fn(),
  closePermissionDialogMock: vi.fn(),
  continueAfterPermissionsGrantedMock: vi.fn(),
  loadPlatformMock: vi.fn(),
  loadConfigMock: vi.fn(),
  initializePermissionsMock: vi.fn(),
  useQuitFlowMock: vi.fn(),
}));

let currentWindowLabel: 'main' | 'preview' = 'main';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: vi.fn(() => ({ label: currentWindowLabel })),
}));

vi.mock('../../stores/serverStore', () => ({
  useServerStore: () => ({
    showPermissionDialog: false,
    closePermissionDialog: closePermissionDialogMock,
    continueAfterPermissionsGranted: continueAfterPermissionsGrantedMock,
  }),
}));

vi.mock('../../hooks/useQuitFlow', () => ({
  useQuitFlow: useQuitFlowMock,
}));

useQuitFlowMock.mockImplementation(() => ({
    showQuitDialog: false,
    closeQuitDialog: closeQuitDialogMock,
    handleQuitConfirm: handleQuitConfirmMock,
}));

vi.mock('../../services/server-events', () => ({
  initializeServerEvents: initializeServerEventsMock,
}));

vi.mock('../../stores/configStore', () => ({
  useConfigStore: () => ({
    activeTab: 'home',
    loadConfig: loadConfigMock,
    loadPlatform: loadPlatformMock,
    platform: 'android',
  }),
}));

vi.mock('../../stores/permissionStore', () => ({
  usePermissionStore: (selector: (state: { initialize: () => void }) => unknown) => selector({ initialize: initializePermissionsMock }),
}));

vi.mock('../ServerCard', () => ({ ServerCard: () => <div>ServerCard</div> }));
vi.mock('../StatsCard', () => ({ StatsCard: () => <div>StatsCard</div> }));
vi.mock('../InfoCard', () => ({ InfoCard: () => <div>InfoCard</div> }));
vi.mock('../LatestPhotoCard', () => ({ LatestPhotoCard: () => <div>LatestPhotoCard</div> }));
vi.mock('../ConfigCard', () => ({ ConfigCard: () => <div>ConfigCard</div> }));
vi.mock('../GalleryCard', () => ({ GalleryCard: () => <div>GalleryCard</div> }));
vi.mock('../BottomNav', () => ({ BottomNav: () => <div>BottomNav</div> }));
vi.mock('../PermissionDialog', () => ({ PermissionDialog: () => null }));
vi.mock('../PreviewWindow', () => ({ PreviewWindow: () => <div>PreviewWindow</div> }));

async function flush(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}

describe('App bootstrap characterization', () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);
    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
    initializeServerEventsMock.mockReset();
    closePermissionDialogMock.mockReset();
    closeQuitDialogMock.mockReset();
    continueAfterPermissionsGrantedMock.mockReset();
    handleQuitConfirmMock.mockReset();
    loadPlatformMock.mockReset();
    loadConfigMock.mockReset();
    initializePermissionsMock.mockReset();
    useQuitFlowMock.mockClear();
    initializeServerEventsMock.mockResolvedValue(vi.fn());
    document.documentElement.className = '';
    currentWindowLabel = 'main';
  });

  afterEach(() => {
    act(() => {
      root.unmount();
    });
    container.remove();
    vi.unstubAllGlobals();
    document.documentElement.className = '';
  });

  it('runs bootstrap effects and calls returned listener cleanup on unmount', async () => {
    const cleanupMock = vi.fn();
    initializeServerEventsMock.mockResolvedValue(cleanupMock);

    await act(async () => {
      root.render(<App />);
      await flush();
    });

    expect(loadPlatformMock).toHaveBeenCalledTimes(1);
    expect(initializePermissionsMock).toHaveBeenCalledTimes(1);
    expect(initializeServerEventsMock).toHaveBeenCalledTimes(1);
    expect(loadConfigMock).toHaveBeenCalledTimes(1);
    expect(useQuitFlowMock).toHaveBeenCalledWith({ enabled: true });
    expect(document.documentElement.className).toBe('platform-android');

    act(() => {
      root.unmount();
    });

    expect(cleanupMock).toHaveBeenCalledTimes(1);
  });

  it('calls cleanup when listener setup resolves after unmount', async () => {
    let resolveSetup: ((cleanup: () => void) => void) | undefined;
    const cleanupMock = vi.fn();
    initializeServerEventsMock.mockReturnValue(
      new Promise((resolve) => {
        resolveSetup = resolve;
      }),
    );

    await act(async () => {
      root.render(<App />);
      await flush();
    });

    act(() => {
      root.unmount();
    });

    await act(async () => {
      resolveSetup?.(cleanupMock);
      await flush();
    });

    expect(cleanupMock).toHaveBeenCalledTimes(1);
  });

  it('skips main bootstrap effects in preview window', async () => {
    currentWindowLabel = 'preview';

    await act(async () => {
      root.render(<App />);
      await flush();
    });

    expect(container.textContent).toContain('PreviewWindow');
    expect(loadPlatformMock).not.toHaveBeenCalled();
    expect(initializePermissionsMock).not.toHaveBeenCalled();
    expect(initializeServerEventsMock).not.toHaveBeenCalled();
    expect(loadConfigMock).not.toHaveBeenCalled();
    expect(useQuitFlowMock).toHaveBeenCalledWith({ enabled: false });
  });
});
