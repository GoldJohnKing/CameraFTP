/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ConfigCard } from '../ConfigCard';
import type { AppConfig } from '../../types';
import { useConfigStore } from '../../stores/configStore';

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

vi.mock('../../stores/permissionStore', () => ({
  usePermissionStore: () => ({
    storageInfo: null,
    needsPermission: false,
    ensureStorageReady: vi.fn(),
    checkPermissions: vi.fn(),
  }),
}));

vi.mock('../../stores/serverStore', () => ({
  useServerStore: () => ({
    isRunning: false,
  }),
}));

vi.mock('../PermissionList', () => ({ PermissionList: () => <div>PermissionList</div> }));
vi.mock('../PathSelector', () => ({ PathSelector: () => <div>PathSelector</div> }));
vi.mock('../AdvancedConnectionConfig', () => ({
  AdvancedConnectionConfigPanel: () => <div>AdvancedConnectionConfigPanel</div>,
}));
vi.mock('../AutoStartToggle', () => ({ AutoStartToggle: () => <div>AutoStartToggle</div> }));
vi.mock('../PreviewConfigCard', () => ({ PreviewConfigCard: () => <div>PreviewConfigCard</div> }));
vi.mock('../AboutCard', () => ({ AboutCard: () => <div>AboutCard</div> }));

import { flush } from '../../test-utils/flush';

describe('ConfigCard Android image viewer settings', () => {
  let container: HTMLDivElement;
  let root: Root;

  const renderCard = async () => {
    await act(async () => {
      root.render(<ConfigCard />);
      await flush();
    });
  };

  beforeEach(() => {
    vi.useFakeTimers();
    vi.stubGlobal('IS_REACT_ACT_ENVIRONMENT', true);
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(false);

    const baseDraft: AppConfig = {
      savePath: '/tmp/cameraftp',
      port: 2121,
      autoSelectPort: false,
      advancedConnection: {
        enabled: false,
        auth: {
          anonymous: true,
          username: '',
          passwordHash: '',
        },
      },
      previewConfig: null,
      androidImageViewer: {
        openMethod: 'built-in-viewer',
        autoOpenLatestWhenVisible: true,
      },
    };

    useConfigStore.setState((state) => ({
      ...state,
      config: baseDraft,
      draft: baseDraft,
      isLoading: false,
      error: null,
      platform: 'android',
      activeTab: 'config',
      draftRevision: 0,
    }));

    container = document.createElement('div');
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => {
      vi.runOnlyPendingTimers();
    });
    act(() => {
      root.unmount();
    });
    useConfigStore.setState((state) => ({
      ...state,
      config: null,
      draft: null,
      error: null,
      isLoading: false,
      draftRevision: 0,
    }));
    container.remove();
    vi.unstubAllGlobals();
    vi.useRealTimers();
  });

  it('shows auto-open toggle in built-in mode and hides it after switching to external app mode', async () => {
    await renderCard();

    expect(container.textContent).toContain('使用外部应用打开图片');
    expect(container.textContent).toContain('自动预览');
    expect(container.textContent).toContain('收到新图片后自动显示预览');

    const externalViewerToggle = container.querySelector(
      'button[aria-label="使用外部应用打开图片"]',
    ) as HTMLButtonElement | null;
    expect(externalViewerToggle).toBeTruthy();

    await act(async () => {
      externalViewerToggle?.click();
      await flush();
    });

    expect(useConfigStore.getState().draft?.androidImageViewer?.openMethod).toBe('external-app');
    expect(container.textContent).not.toContain('自动预览');
    expect(container.textContent).not.toContain('收到新图片后自动显示预览');
  });
});
