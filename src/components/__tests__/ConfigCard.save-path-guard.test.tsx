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
import { flush } from '../../test-utils/flush';

// PathSelector 不 mock：钉住 ConfigCard → PathSelector 的真实接线
//（isRunning → disabled + 可见原因提示）

const { invokeMock, serverState } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  serverState: { isRunning: false },
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

vi.mock('../../stores/permissionStore', () => {
  const state = {
    storageInfo: null,
    needsPermission: false,
    ensureStorageReady: vi.fn(),
    checkPermissions: vi.fn(),
  };
  return {
    usePermissionStore: (selector?: (s: typeof state) => unknown) =>
      selector ? selector(state) : state,
  };
});

vi.mock('../../stores/serverStore', () => ({
  useServerStore: (selector?: (s: typeof serverState) => unknown) =>
    selector ? selector(serverState) : serverState,
}));

vi.mock('../PermissionList', () => ({ PermissionList: () => <div>PermissionList</div> }));
vi.mock('../AdvancedConnectionConfig', () => ({
  AdvancedConnectionConfigPanel: () => <div>AdvancedConnectionConfigPanel</div>,
}));
vi.mock('../PreviewConfigCard', () => ({ PreviewConfigCard: () => <div>PreviewConfigCard</div> }));
vi.mock('../AutoColorGradingConfigCard', () => ({
  AutoColorGradingConfigCard: () => <div>AutoColorGradingConfigCard</div>,
}));
vi.mock('../AboutCard', () => ({ AboutCard: () => <div>AboutCard</div> }));

const SAVE_PATH_HINT = 'FTP 服务器运行中，请先停止服务器再更改存储路径';

describe('ConfigCard save-path picker while server running', () => {
  const { getContainer, getRoot } = setupReactRoot();

  const renderCard = async () => {
    await act(async () => {
      getRoot().render(<ConfigCard />);
      await flush();
    });
  };

  const findChangeButton = (): HTMLButtonElement | undefined =>
    Array.from(getContainer().querySelectorAll('button')).find(
      (button) => button.textContent?.trim() === '更改',
    ) as HTMLButtonElement | undefined;

  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(false);
    serverState.isRunning = false;

    const baseDraft: AppConfig = {
      savePath: 'D:/photos/cameraftp',
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
      androidImageViewer: null,
      aiEdit: {
        autoEdit: true,
        prompt: '',
        manualPrompt: '',
        manualModel: '',
        provider: { type: 'seed-edit', apiKey: '', model: 'doubao-seedream-5-0-260128' },
      },
      autoColorGrading: null,
      colorGradingLastUsed: null,
    };

    useConfigStore.setState((state) => ({
      ...state,
      config: baseDraft,
      draft: baseDraft,
      isLoading: false,
      error: null,
      platform: 'windows',
      activeTab: 'config',
    }));
  });

  afterEach(() => {
    useConfigStore.setState((state) => ({
      ...state,
      config: null,
      draft: null,
      error: null,
      isLoading: false,
    }));
    serverState.isRunning = false;
  });

  it('disables the save-path picker with a visible reason while the server is running', async () => {
    serverState.isRunning = true;
    await renderCard();

    const changeButton = findChangeButton();
    expect(changeButton).toBeTruthy();
    expect(changeButton?.disabled).toBe(true);
    expect(getContainer().textContent).toContain(SAVE_PATH_HINT);

    // 禁用态点击不得触发目录选择
    await act(async () => {
      changeButton?.click();
      await flush();
    });
    expect(invokeMock).not.toHaveBeenCalledWith('select_save_directory');
  });

  it('keeps the picker enabled without a hint while the server is stopped', async () => {
    serverState.isRunning = false;
    await renderCard();

    const changeButton = findChangeButton();
    expect(changeButton).toBeTruthy();
    expect(changeButton?.disabled).toBe(false);
    expect(getContainer().textContent).not.toContain(SAVE_PATH_HINT);
  });
});
