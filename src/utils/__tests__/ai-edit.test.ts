/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { describe, it, expect } from 'vitest';
import { getAiEditCallContext } from '../ai-edit';
import {
  SEEDREAM_MODELS,
  DEFAULT_SEEDREAM_MODEL,
} from '../../../src-tauri/bindings/SeedreamModels';
import type { AppConfig } from '../../types';

describe('getAiEditCallContext', () => {
  it('exposes the seedream model list for the native dialog dropdown', () => {
    const ctx = getAiEditCallContext(null);

    expect(ctx.models).toEqual(
      SEEDREAM_MODELS.map((m) => ({ value: m.value, label: m.label }))
    );
  });

  it('models entries have the { value, label } shape required by native', () => {
    const ctx = getAiEditCallContext(undefined);

    expect(ctx.models.length).toBeGreaterThan(0);
    for (const m of ctx.models) {
      expect(typeof m.value).toBe('string');
      expect(m.value.length).toBeGreaterThan(0);
      expect(typeof m.label).toBe('string');
      expect(m.label.length).toBeGreaterThan(0);
    }
  });

  it('keeps the default model selectable in the exposed list', () => {
    const ctx = getAiEditCallContext(null);

    expect(ctx.models.some((m) => m.value === DEFAULT_SEEDREAM_MODEL)).toBe(
      true
    );
  });

  it('generated catalog keeps the default model as its first entry', () => {
    expect(SEEDREAM_MODELS[0].value).toBe(DEFAULT_SEEDREAM_MODEL);
  });

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
