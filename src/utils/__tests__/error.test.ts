/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { describe, expect, it } from 'vitest';
import { formatError, silent } from '../error';

describe('formatError', () => {
  it('returns message for Error instances', () => {
    expect(formatError(new Error('something went wrong'))).toBe('something went wrong');
  });

  it('returns string as-is', () => {
    expect(formatError('plain string')).toBe('plain string');
  });

  it('returns "Unknown error" for null', () => {
    expect(formatError(null)).toBe('Unknown error');
  });

  it('returns "Unknown error" for undefined', () => {
    expect(formatError(undefined)).toBe('Unknown error');
  });

  it('JSON-stringifies objects', () => {
    expect(formatError({ code: 42, detail: 'bad' })).toBe('{"code":42,"detail":"bad"}');
  });

  it('extracts userMessage from Tauri-style error objects', () => {
    expect(formatError({ code: 'AI_EDIT_ERROR', message: 'raw', userMessage: '用户友好的错误信息', isCritical: false }))
      .toBe('用户友好的错误信息');
  });

  it('extracts message from objects without userMessage', () => {
    expect(formatError({ code: 'ERR', message: 'something failed' }))
      .toBe('something failed');
  });

  it('falls back to String() for non-serializable objects', () => {
    const circular: Record<string, unknown> = {};
    circular.self = circular;
    // JSON.stringify throws on circular references
    const result = formatError(circular);
    expect(result).toContain('[object Object]');
    // String() on a plain object returns "[object Object]"
  });

  it('handles numbers', () => {
    expect(formatError(404)).toBe('404');
  });
});

describe('silent', () => {
  it('returns result on success', async () => {
    const result = await silent(() => Promise.resolve('ok'));
    expect(result).toBe('ok');
  });

  it('returns null on error', async () => {
    const result = await silent(() => Promise.reject(new Error('fail')));
    expect(result).toBeNull();
  });
});
