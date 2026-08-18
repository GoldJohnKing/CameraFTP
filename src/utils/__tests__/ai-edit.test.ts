/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { describe, it, expect } from 'vitest';
import { getAiEditCallContext } from '../ai-edit';
import { SEEDREAM_MODELS } from '../../../src-tauri/bindings/SeedreamModels';
import type { AppConfig } from '../../types';

describe('getAiEditCallContext', () => {
  it('dialog prompt never falls back to the auto-edit prompt (ruling #4)', () => {
    // manualPrompt empty + aiEdit.prompt set: the dialog default must stay
    // empty instead of borrowing the auto-edit prompt.
    const draft = {
      aiEdit: {
        prompt: 'auto-edit prompt',
        manualPrompt: '',
        provider: { type: 'seed-edit' as const, apiKey: 'k' },
      },
    } as unknown as AppConfig;

    const ctx = getAiEditCallContext(draft);

    expect(ctx.dialogPrompt).toBe('');
    expect(ctx.autoPrompt).toBe('auto-edit prompt');
  });

  it('auto prompt never falls back to manualPrompt (ruling #4)', () => {
    const draft = {
      aiEdit: {
        prompt: '',
        manualPrompt: 'manual prompt',
        provider: { type: 'seed-edit' as const, apiKey: 'k' },
      },
    } as unknown as AppConfig;

    const ctx = getAiEditCallContext(draft);

    expect(ctx.dialogPrompt).toBe('manual prompt');
    expect(ctx.autoPrompt).toBe('');
  });

  it('exposes the selected model as a selectable list entry', () => {
    // 原生 Android 对话框用 ctx.models 渲染下拉框：配置里选中的模型必须
    // 在列表里，否则下拉框会出现无匹配项的悬空值。空配置时 ctx.model 为
    // ''，默认值由原生对话框自行处理（前端 util 不做默认模型回退）。
    const draft = {
      aiEdit: {
        prompt: '',
        manualPrompt: '',
        manualModel: SEEDREAM_MODELS[0].value,
        provider: { type: 'seed-edit' as const, apiKey: 'k' },
      },
    } as unknown as AppConfig;

    const ctx = getAiEditCallContext(draft);

    expect(ctx.models).toEqual(SEEDREAM_MODELS);
    expect(ctx.models.map(m => m.value)).toContain(ctx.model);
  });

  it('carries manual and auto prompts on independent fields when both set', () => {
    const draft = {
      aiEdit: {
        prompt: 'auto prompt',
        manualPrompt: 'manual prompt',
        provider: { type: 'seed-edit' as const, apiKey: 'k' },
      },
    } as unknown as AppConfig;

    const ctx = getAiEditCallContext(draft);

    expect(ctx.dialogPrompt).toBe('manual prompt');
    expect(ctx.autoPrompt).toBe('auto prompt');
  });

  it('serializes prompts and models losslessly into the bridge JSON payload', () => {
    const draft = {
      aiEdit: {
        prompt: 'auto prompt',
        manualPrompt: 'enhance',
        manualModel: 'doubao-seedream-4-5-251128',
        autoEdit: true,
        provider: { type: 'seed-edit' as const, apiKey: 'k' },
      },
    } as unknown as AppConfig;
    const payload = JSON.parse(
      JSON.stringify(getAiEditCallContext(draft))
    ) as ReturnType<typeof getAiEditCallContext>;

    expect(payload.dialogPrompt).toBe('enhance');
    expect(payload.autoPrompt).toBe('auto prompt');
    expect(payload.model).toBe('doubao-seedream-4-5-251128');
    expect(payload.models).toEqual(SEEDREAM_MODELS);
    expect(payload.autoEdit).toBe(true);
    expect(payload.hasApiKey).toBe(true);
  });
});
