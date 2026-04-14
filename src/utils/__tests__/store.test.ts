/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { debounce, executeAsync } from '../store';

describe('debounce', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('delays execution', () => {
    const fn = vi.fn();
    const debounced = debounce(fn, 100);

    debounced('a');

    expect(fn).not.toHaveBeenCalled();
    vi.advanceTimersByTime(100);
    expect(fn).toHaveBeenCalledTimes(1);
    expect(fn).toHaveBeenCalledWith('a');
  });

  it('calls with latest args after delay', () => {
    const fn = vi.fn();
    const debounced = debounce(fn, 100);

    debounced('a');
    debounced('b');
    debounced('c');

    vi.advanceTimersByTime(100);
    expect(fn).toHaveBeenCalledTimes(1);
    expect(fn).toHaveBeenCalledWith('c');
  });

  it('cancel() prevents execution', () => {
    const fn = vi.fn();
    const debounced = debounce(fn, 100);

    debounced('a');
    debounced.cancel();
    vi.advanceTimersByTime(100);

    expect(fn).not.toHaveBeenCalled();
  });

  it('flush() executes immediately with latest args', () => {
    const fn = vi.fn();
    const debounced = debounce(fn, 100);

    debounced('a');
    debounced('b');
    debounced.flush();

    expect(fn).toHaveBeenCalledTimes(1);
    expect(fn).toHaveBeenCalledWith('b');

    // No second execution after timer fires
    vi.advanceTimersByTime(100);
    expect(fn).toHaveBeenCalledTimes(1);
  });

  it('handles multiple rapid calls by executing only once', () => {
    const fn = vi.fn();
    const debounced = debounce(fn, 50);

    for (let i = 0; i < 10; i++) {
      debounced(i);
    }

    vi.advanceTimersByTime(50);
    expect(fn).toHaveBeenCalledTimes(1);
    expect(fn).toHaveBeenCalledWith(9);
  });
});

describe('executeAsync', () => {
  it('sets isLoading true during operation', async () => {
    const states: unknown[] = [];
    const set = (fn: (state: Record<string, unknown>) => Record<string, unknown>) => {
      const current = { isLoading: false, error: null };
      const next = fn(current);
      states.push({ ...next });
    };

    await executeAsync(
      {
        operation: () => Promise.resolve('result'),
        onSuccess: vi.fn(),
      },
      set,
    );

    expect(states[0]).toEqual({ isLoading: true, error: null });
  });

  it('calls onSuccess with result', async () => {
    const onSuccess = vi.fn();
    const set = vi.fn((fn: (state: Record<string, unknown>) => Record<string, unknown>) => fn({}));

    await executeAsync(
      {
        operation: () => Promise.resolve('data'),
        onSuccess,
      },
      set,
    );

    expect(onSuccess).toHaveBeenCalledTimes(1);
    expect(onSuccess).toHaveBeenCalledWith('data', set);
  });

  it('sets error message on failure', async () => {
    const states: unknown[] = [];
    const set = (fn: (state: Record<string, unknown>) => Record<string, unknown>) => {
      states.push(fn({}));
    };

    await executeAsync(
      {
        operation: () => Promise.reject(new Error('network')),
        onSuccess: vi.fn(),
      },
      set,
    );

    // states[0] = isLoading:true, states[1] = error message, states[2] = isLoading:false
    const errorState = states[1] as Record<string, unknown>;
    expect(errorState.error).toBe('network');
  });

  it('prepends errorPrefix to error message', async () => {
    const states: unknown[] = [];
    const set = (fn: (state: Record<string, unknown>) => Record<string, unknown>) => {
      states.push(fn({}));
    };

    await executeAsync(
      {
        operation: () => Promise.reject(new Error('timeout')),
        onSuccess: vi.fn(),
        errorPrefix: 'Upload failed',
      },
      set,
    );

    const errorState = states[1] as Record<string, unknown>;
    expect(errorState.error).toBe('Upload failed: timeout');
  });

  it('sets isLoading false in finally', async () => {
    const states: unknown[] = [];
    const set = (fn: (state: Record<string, unknown>) => Record<string, unknown>) => {
      const current = { isLoading: false, error: null };
      states.push(fn(current));
    };

    await executeAsync(
      {
        operation: () => Promise.resolve('ok'),
        onSuccess: vi.fn(),
      },
      set,
    );

    const lastState = states[states.length - 1] as Record<string, unknown>;
    expect(lastState.isLoading).toBe(false);
  });

  it('rethrows when rethrow: true', async () => {
    const set = vi.fn((fn: (state: Record<string, unknown>) => Record<string, unknown>) => fn({}));

    await expect(
      executeAsync(
        {
          operation: () => Promise.reject(new Error('fatal')),
          onSuccess: vi.fn(),
          rethrow: true,
        },
        set,
      ),
    ).rejects.toThrow('fatal');
  });

  it('returns undefined on failure without rethrow', async () => {
    const set = vi.fn((fn: (state: Record<string, unknown>) => Record<string, unknown>) => fn({}));

    const result = await executeAsync(
      {
        operation: () => Promise.reject(new Error('fail')),
        onSuccess: vi.fn(),
      },
      set,
    );

    expect(result).toBeUndefined();
  });
});
