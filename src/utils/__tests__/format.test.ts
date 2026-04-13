/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { formatBytes, withMinDuration } from '../format';

describe('formatBytes', () => {
  it('returns "0 MB" for zero', () => {
    expect(formatBytes(0)).toBe('0 MB');
  });

  it('formats 1 MB correctly', () => {
    expect(formatBytes(1024 * 1024)).toBe('1.0 MB');
  });

  it('formats fractional MB values', () => {
    expect(formatBytes(512 * 1024)).toBe('0.5 MB');
  });

  it('formats 12.5 MB correctly', () => {
    expect(formatBytes(12.5 * 1024 * 1024)).toBe('12.5 MB');
  });

  it('formats very large values', () => {
    expect(formatBytes(1024 * 1024 * 1024)).toBe('1024.0 MB');
  });
});

describe('withMinDuration', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('resolves with the function result', async () => {
    const result = await withMinDuration(() => Promise.resolve(42), 0);
    expect(result).toBe(42);
  });

  it('waits at least minMs when fn resolves faster', async () => {
    const start = Date.now();
    const promise = withMinDuration(() => Promise.resolve('done'), 100);
    await vi.advanceTimersByTimeAsync(100);
    await promise;
    expect(Date.now() - start).toBeGreaterThanOrEqual(100);
  });

  it('uses default 200ms when minMs not specified', async () => {
    const promise = withMinDuration(() => Promise.resolve('done'));
    await vi.advanceTimersByTimeAsync(200);
    const result = await promise;
    expect(result).toBe('done');
  });

  it('still rejects if fn rejects', async () => {
    const error = new Error('boom');
    const promise = withMinDuration(() => Promise.reject(error), 50);
    // Prevent unhandled rejection during timer advancement
    promise.catch(() => {});
    await vi.advanceTimersByTimeAsync(50);
    await expect(promise).rejects.toThrow('boom');
  });

  it('waits minMs even when fn rejects', async () => {
    const error = new Error('fail');
    const promise = withMinDuration(() => Promise.reject(error), 100);
    promise.catch(() => {});
    await vi.advanceTimersByTimeAsync(100);
    await expect(promise).rejects.toThrow('fail');
  });
});
