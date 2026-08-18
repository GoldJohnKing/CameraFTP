/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { act } from 'react';
import { fireEvent } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { flush } from '../../test-utils/flush';
import { setupReactRoot } from '../../test-utils/react-root';
import type { AppConfig } from '../../types';

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

import { AiEditConfigPanel } from '../AiEditConfigPanel';

function makeConfig(overrides: {
  apiKey?: string;
  prompt?: string;
  autoEdit?: boolean;
} = {}): AppConfig {
  return {
    savePath: '/tmp/cameraftp',
    port: 2121,
    autoSelectPort: false,
    advancedConnection: {
      enabled: false,
      auth: { anonymous: true, username: '', passwordHash: '' },
    },
    previewConfig: null,
    androidImageViewer: null,
    aiEdit: {
      autoEdit: overrides.autoEdit ?? true,
      prompt: overrides.prompt ?? '默认提示词',
      manualPrompt: '',
      manualModel: '',
      provider: {
        type: 'seed-edit',
        apiKey: overrides.apiKey ?? 'key-a',
        model: 'doubao-seedream-5-0-260128',
      },
    },
    autoColorGrading: null,
    colorGradingLastUsed: null,
  };
}

describe('AiEditConfigPanel local state sync', () => {
  const { getContainer, getRoot } = setupReactRoot();

  const updateDraft = vi.fn();

  const renderPanel = async (config: AppConfig) => {
    await act(async () => {
      getRoot().render(
        <AiEditConfigPanel config={config} isLoading={false} updateDraft={updateDraft} />,
      );
      await flush();
    });
  };

  const getApiKeyInput = (): HTMLInputElement =>
    getContainer().querySelector('input[autocomplete="off"]') as HTMLInputElement;

  const getEyeButton = (): HTMLButtonElement =>
    getContainer().querySelector('div.relative > button') as HTMLButtonElement;

  const getPromptTextarea = (): HTMLTextAreaElement =>
    getContainer().querySelector('textarea[placeholder="请输入提示词"]') as HTMLTextAreaElement;

  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue(undefined);
    updateDraft.mockReset();
  });

  it('resets the local API key input when the provider config changes from outside', async () => {
    await renderPanel(makeConfig({ apiKey: 'key-a' }));

    // Reveal the value so the real (unmasked) state is observable.
    await act(async () => {
      getEyeButton().click();
      await flush();
    });
    expect(getApiKeyInput().value).toBe('key-a');

    // Parent swaps in a config whose provider object is new (e.g. loaded
    // from disk after another edit) with a different key.
    await renderPanel(makeConfig({ apiKey: 'key-b' }));
    expect(getApiKeyInput().value).toBe('key-b');

    // And again with an empty key.
    await renderPanel(makeConfig({ apiKey: '' }));
    expect(getApiKeyInput().value).toBe('');
  });

  it('keeps masking the API key until the eye toggle reveals it', async () => {
    await renderPanel(makeConfig({ apiKey: 'key-a' }));
    const dot = '\u2022';
    expect(getApiKeyInput().value).toBe(dot.repeat(5));
  });

  it('syncs the local prompt input when config.aiEdit.prompt changes from outside', async () => {
    await renderPanel(makeConfig({ prompt: 'first prompt' }));
    expect(getPromptTextarea().value).toBe('first prompt');

    await renderPanel(makeConfig({ prompt: 'second prompt' }));
    expect(getPromptTextarea().value).toBe('second prompt');
  });

  it('defers prompt updates: editing does not commit until blur', async () => {
    const config = makeConfig({ prompt: 'original' });
    await renderPanel(config);

    await act(async () => {
      fireEvent.change(getPromptTextarea(), { target: { value: 'edited prompt' } });
      await flush();
    });
    expect(updateDraft).not.toHaveBeenCalled();

    await act(async () => {
      fireEvent.blur(getPromptTextarea());
      await flush();
    });
    expect(updateDraft).toHaveBeenCalledTimes(1);

    const updater = updateDraft.mock.calls[0][0] as (draft: AppConfig) => AppConfig;
    const next = updater(config);
    expect(next.aiEdit.prompt).toBe('edited prompt');
  });

  it('does not commit the prompt when blur leaves it unchanged', async () => {
    await renderPanel(makeConfig({ prompt: 'original' }));

    await act(async () => {
      fireEvent.blur(getPromptTextarea());
      await flush();
    });
    expect(updateDraft).not.toHaveBeenCalled();
  });

  it('commits the masked API key on blur after an edit', async () => {
    const config = makeConfig({ apiKey: 'key-a' });
    await renderPanel(config);

    // Masked display is '•••••'; append 'X' at the end → payload 'key-aX'.
    await act(async () => {
      fireEvent.change(getApiKeyInput(), {
        target: { value: '\u2022\u2022\u2022\u2022\u2022X' },
      });
      await flush();
    });
    expect(updateDraft).not.toHaveBeenCalled();

    await act(async () => {
      fireEvent.blur(getApiKeyInput());
      await flush();
    });
    expect(updateDraft).toHaveBeenCalledTimes(1);

    const updater = updateDraft.mock.calls[0][0] as (draft: AppConfig) => AppConfig;
    const next = updater(config);
    expect(next.aiEdit.provider).toMatchObject({ type: 'seed-edit', apiKey: 'key-aX' });
  });

  it('toggles autoEdit through updateDraft', async () => {
    const config = makeConfig({ autoEdit: false });
    await renderPanel(config);

    const toggle = getContainer().querySelector('button[aria-label="自动修图"]') as HTMLButtonElement;
    expect(toggle).toBeTruthy();

    await act(async () => {
      toggle.click();
      await flush();
    });

    expect(updateDraft).toHaveBeenCalledTimes(1);
    const updater = updateDraft.mock.calls[0][0] as (draft: AppConfig) => AppConfig;
    const next = updater(config);
    expect(next.aiEdit.autoEdit).toBe(true);
  });
});
