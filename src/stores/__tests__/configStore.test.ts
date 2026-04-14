/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { AppConfig, PreviewWindowConfig } from '../../types';
import { useConfigStore } from '../configStore';

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

function createDeferred() {
  let resolvePromise: () => void = () => {};
  const promise = new Promise<void>((resolve) => {
    resolvePromise = resolve;
  });

  return {
    promise,
    resolve: resolvePromise,
  };
}

const baseConfig: AppConfig = {
  savePath: '/tmp/cameraftp',
  port: 2121,
  autoSelectPort: false,
  advancedConnection: {
    enabled: true,
    auth: {
      anonymous: false,
      username: 'camera',
      passwordHash: 'hash',
    },
  },
  previewConfig: {
    enabled: true,
    method: 'built-in-preview',
    customPath: null,
    autoBringToFront: false,
  },
  androidImageViewer: null,
};

describe('configStore coordination', () => {
  beforeEach(() => {
    vi.useFakeTimers();
    invokeMock.mockReset();
    useConfigStore.setState({
      config: baseConfig,
      draft: baseConfig,
      isLoading: false,
      error: null,
      activeTab: 'home',
      platform: 'windows',
    });
  });

  it('waits for pending whole-config save before saveAuthConfig', async () => {
    const deferredSave = createDeferred();

    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'save_config') {
        await deferredSave.promise;
        return null;
      }
      if (command === 'save_auth_config') return null;
      if (command === 'load_config') return baseConfig;
      return null;
    });

    useConfigStore.getState().updateDraft((draft) => ({ ...draft, port: 3000 }));
    const saveAuthPromise = useConfigStore.getState().saveAuthConfig({
      anonymous: false,
      username: 'camera',
      password: 'secret',
    });

    await Promise.resolve();
    expect(invokeMock).toHaveBeenCalledWith('save_config', expect.any(Object));
    expect(invokeMock).not.toHaveBeenCalledWith('save_auth_config', expect.any(Object));

    deferredSave.resolve();
    await saveAuthPromise;

    expect(invokeMock).toHaveBeenCalledWith('save_auth_config', {
      anonymous: false,
      username: 'camera',
      password: 'secret',
    });
  });

  it('waits for in-flight whole-config save before updatePreviewConfig', async () => {
    const deferredSave = createDeferred();

    invokeMock.mockImplementation(async (command: string, payload?: { patch?: Record<string, unknown> }) => {
      if (command === 'save_config') {
        await deferredSave.promise;
        return null;
      }
      if (command === 'update_preview_config') {
        return {
          ...baseConfig.previewConfig,
          ...(payload?.patch ?? {}),
        };
      }
      if (command === 'load_config') return baseConfig;
      return null;
    });

    useConfigStore.getState().updateDraft((draft) => ({ ...draft, savePath: '/tmp/new-path' }));
    await vi.advanceTimersByTimeAsync(100);

    const savePreviewPromise = useConfigStore.getState().updatePreviewConfig({ autoBringToFront: true });

    await Promise.resolve();
    expect(invokeMock).not.toHaveBeenCalledWith('update_preview_config', expect.any(Object));

    deferredSave.resolve();
    await savePreviewPromise;

    expect(invokeMock).toHaveBeenCalledWith('update_preview_config', {
      patch: {
        autoBringToFront: true,
      },
    });
    expect(invokeMock).not.toHaveBeenCalledWith('set_preview_config', expect.any(Object));
  });

  it('skips stale whole-config saves queued before or during narrow auth save', async () => {
    const authSaveDeferred = createDeferred();
    const backendConfigAfterAuth: AppConfig = {
      ...baseConfig,
      advancedConnection: {
        ...baseConfig.advancedConnection,
        auth: {
          ...baseConfig.advancedConnection.auth,
          username: 'narrow-user',
        },
      },
    };

    invokeMock.mockImplementation(async (command: string, payload?: unknown) => {
      if (command === 'save_config') return null;
      if (command === 'save_auth_config') {
        await authSaveDeferred.promise;
        return null;
      }
      if (command === 'load_config') return backendConfigAfterAuth;
      if (command === 'update_preview_config') return baseConfig.previewConfig;
      return payload;
    });

    useConfigStore.getState().updateDraft((draft) => ({ ...draft, port: 3000 }));
    const saveAuthPromise = useConfigStore.getState().saveAuthConfig({
      anonymous: false,
      username: 'narrow-user',
      password: 'secret',
    });

    await Promise.resolve();
    useConfigStore.getState().updateDraft((draft) => ({ ...draft, savePath: '/tmp/new-path' }));
    await vi.advanceTimersByTimeAsync(100);

    authSaveDeferred.resolve();
    await saveAuthPromise;
    await Promise.resolve();

    const saveConfigCalls = invokeMock.mock.calls.filter(([command]) => command === 'save_config');
    expect(saveConfigCalls).toHaveLength(1);

    expect(useConfigStore.getState().config?.advancedConnection.auth.username).toBe('narrow-user');
  });

  it('does not preserve stale advancedConnection enabled when only auth changed locally during full resync', async () => {
    const backendConfigAfterAuth: AppConfig = {
      ...baseConfig,
      advancedConnection: {
        enabled: false,
        auth: {
          ...baseConfig.advancedConnection.auth,
          username: 'backend-user',
        },
      },
    };

    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'update_preview_config') return {
        ...baseConfig.previewConfig,
        autoBringToFront: true,
      };
      if (command === 'load_config') return backendConfigAfterAuth;
      return null;
    });

    useConfigStore.setState((state) => ({
      ...state,
      config: baseConfig,
      draft: {
        ...baseConfig,
        advancedConnection: {
          ...baseConfig.advancedConnection,
          auth: {
            ...baseConfig.advancedConnection.auth,
            username: 'draft-user',
          },
        },
      },
    }));

    await useConfigStore.getState().updatePreviewConfig({ autoBringToFront: true });

    expect(useConfigStore.getState().draft?.advancedConnection.enabled).toBe(false);
    expect(useConfigStore.getState().draft?.advancedConnection.auth.username).toBe('draft-user');
  });

  it('serializes overlapping preview saves to avoid clobbering', async () => {
    const firstPreviewSaveDeferred = createDeferred();
    let previewConfig = { ...baseConfig.previewConfig };

    invokeMock.mockImplementation(async (command: string, payload?: { patch?: Partial<typeof previewConfig> }) => {
      if (command === 'update_preview_config') {
        if (payload?.patch?.autoBringToFront) {
          await firstPreviewSaveDeferred.promise;
        }
        if (payload?.patch) {
          previewConfig = { ...previewConfig, ...payload.patch };
        }
        return previewConfig;
      }
      if (command === 'load_config') {
        return {
          ...baseConfig,
          previewConfig,
        };
      }
      return null;
    });

    const firstSavePromise = useConfigStore.getState().updatePreviewConfig({ autoBringToFront: true });
    const secondSavePromise = useConfigStore.getState().updatePreviewConfig({ enabled: false });

    firstPreviewSaveDeferred.resolve();
    await Promise.all([firstSavePromise, secondSavePromise]);

    const previewSetCalls = invokeMock.mock.calls.filter(([command]) => command === 'update_preview_config');
    expect(previewSetCalls).toHaveLength(2);
    expect(previewSetCalls[0]).toEqual([
      'update_preview_config',
      {
        patch: {
          autoBringToFront: true,
        },
      },
    ]);

    expect(invokeMock).toHaveBeenCalledWith('update_preview_config', {
      patch: {
        enabled: false,
      },
    });
    expect(invokeMock).not.toHaveBeenCalledWith('set_preview_config', expect.any(Object));
  });

  it('applies backend preview updates without clobbering unrelated draft edits', () => {
    useConfigStore.setState((state) => ({
      ...state,
      config: baseConfig,
      draft: {
        ...baseConfig,
        savePath: '/tmp/unsaved-draft-path',
      },
    }));

    const updatedPreviewConfig: PreviewWindowConfig = {
      ...baseConfig.previewConfig!,
      autoBringToFront: true,
    };

    useConfigStore.getState().applyPreviewConfig(updatedPreviewConfig);

    expect(useConfigStore.getState().draft?.savePath).toBe('/tmp/unsaved-draft-path');
    expect(useConfigStore.getState().draft?.previewConfig?.autoBringToFront).toBe(true);
    expect(useConfigStore.getState().config?.previewConfig?.autoBringToFront).toBe(true);
  });

  it('keeps autoOpenLatestWhenVisible true when updateDraft only changes android openMethod', () => {
    const androidConfig: AppConfig = {
      ...baseConfig,
      androidImageViewer: {
        openMethod: 'built-in-viewer',
        autoOpenLatestWhenVisible: true,
      },
    };

    useConfigStore.setState((state) => ({
      ...state,
      config: androidConfig,
      draft: androidConfig,
      platform: 'android',
    }));

    useConfigStore.getState().updateDraft((draft) => ({
      ...draft,
      androidImageViewer: {
        ...draft.androidImageViewer!,
        openMethod: 'external-app',
      },
    }));

    expect(useConfigStore.getState().draft?.androidImageViewer?.openMethod).toBe('external-app');
    expect(useConfigStore.getState().draft?.androidImageViewer?.autoOpenLatestWhenVisible).toBe(true);
  });

});
