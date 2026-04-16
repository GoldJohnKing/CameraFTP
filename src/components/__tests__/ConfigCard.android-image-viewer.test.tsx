/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { act } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ConfigCard } from '../ConfigCard';
import type { AppConfig } from '../../types';
import { useConfigStore } from '../../stores/configStore';
import { setupReactRoot } from '../../test-utils/react-root';

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
vi.mock('../PreviewConfigCard', () => ({ PreviewConfigCard: () => <div>PreviewConfigCard</div> }));
vi.mock('../AboutCard', () => ({ AboutCard: () => <div>AboutCard</div> }));

import { flush } from '../../test-utils/flush';

describe('ConfigCard Android image viewer settings', () => {
  const { getContainer, getRoot } = setupReactRoot();

  const renderCard = async () => {
    await act(async () => {
      getRoot().render(<ConfigCard />);
      await flush();
    });
  };

  beforeEach(() => {
    vi.useFakeTimers();
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
      aiEdit: {
        enabled: false,
        autoEdit: true,
        prompt: '',
        provider: { type: 'seed-edit', apiKey: '' },
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
    }));
  });

  afterEach(() => {
    act(() => {
      vi.runOnlyPendingTimers();
    });
    useConfigStore.setState((state) => ({
      ...state,
      config: null,
      draft: null,
      error: null,
      isLoading: false,
    }));
    vi.useRealTimers();
  });

  it('shows auto-open toggle in built-in mode and hides it after switching to external app mode', async () => {
    await renderCard();

    expect(getContainer().textContent).toContain('使用外部应用打开图片');
    expect(getContainer().textContent).toContain('自动预览');
    expect(getContainer().textContent).toContain('收到新图片后自动显示预览');

    const externalViewerToggle = getContainer().querySelector(
      'button[aria-label="使用外部应用打开图片"]',
    ) as HTMLButtonElement | null;
    expect(externalViewerToggle).toBeTruthy();

    await act(async () => {
      externalViewerToggle?.click();
      await flush();
    });

    expect(useConfigStore.getState().draft?.androidImageViewer?.openMethod).toBe('external-app');
    expect(getContainer().textContent).not.toContain('自动预览');
    expect(getContainer().textContent).not.toContain('收到新图片后自动显示预览');
  });
});
