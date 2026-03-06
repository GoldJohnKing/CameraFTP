/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { formatError } from './error';

/**
 * Creates a debounced version of a function.
 * The returned function delays execution until after `delay` milliseconds
 * have elapsed since the last time it was invoked.
 * 
 * Uses `any[]` to preserve the original function's parameter types through type inference.
 * This is a common and accepted pattern for higher-order utility functions.
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function debounce<T extends (...args: any[]) => any>(
  fn: T,
  delay: number
): T & { cancel: () => void; flush: () => void } {
  let timeoutId: ReturnType<typeof setTimeout> | null = null;
  let lastArgs: Parameters<T> | null = null;

  const debounced = (...args: Parameters<T>) => {
    lastArgs = args;
    if (timeoutId) clearTimeout(timeoutId);
    timeoutId = setTimeout(() => {
      if (lastArgs) fn(...lastArgs);
      timeoutId = null;
      lastArgs = null;
    }, delay);
  };

  debounced.cancel = () => {
    if (timeoutId) clearTimeout(timeoutId);
    timeoutId = null;
    lastArgs = null;
  };

  debounced.flush = () => {
    if (timeoutId && lastArgs) {
      clearTimeout(timeoutId);
      fn(...lastArgs);
      timeoutId = null;
      lastArgs = null;
    }
  };

  return debounced as T & { cancel: () => void; flush: () => void };
}

interface AsyncActionOptions<T, S> {
  operation: () => Promise<T>;
  onSuccess: (result: T, set: (fn: (state: S) => S) => void) => void;
  errorPrefix?: string;
  rethrow?: boolean;
}

interface RetryOptions {
  maxRetries: number;
  delayMs: number;
  maxDurationMs?: number;
}

export function retryAction(
  action: () => boolean | void,
  options: RetryOptions
): void {
  const { maxRetries, delayMs, maxDurationMs = 5000 } = options;
  const startTime = Date.now();
  
  const tryAction = (retriesLeft: number) => {
    if (Date.now() - startTime > maxDurationMs) {
      console.warn(`[retryAction] Max duration (${maxDurationMs}ms) exceeded, giving up`);
      return;
    }
    
    try {
      const result = action();
      if (result === false && retriesLeft > 0) {
        setTimeout(() => tryAction(retriesLeft - 1), delayMs);
      }
    } catch {
      if (retriesLeft > 0) {
        setTimeout(() => tryAction(retriesLeft - 1), delayMs);
      }
    }
  };
  
  tryAction(maxRetries);
}

export async function executeAsync<T, S>(
  options: AsyncActionOptions<T, S>,
  set: (fn: (state: S) => S) => void,
): Promise<T | undefined> {
  const { operation, onSuccess, errorPrefix, rethrow = false } = options;

  set((state) => ({ ...state, isLoading: true, error: null }));

  try {
    const result = await operation();
    onSuccess(result, set);
    return result;
  } catch (err: unknown) {
    let errorMessage = formatError(err);
    if (errorPrefix) {
      errorMessage = `${errorPrefix}: ${errorMessage}`;
    }
    set((state) => ({ ...state, error: errorMessage }));
    if (rethrow) {
      throw err;
    }
    return undefined;
  } finally {
    set((state) => ({ ...state, isLoading: false }));
  }
}
