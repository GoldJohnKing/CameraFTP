/**
 * Zustand store utilities for common async action patterns
 */

import { formatError } from './error';

/**
 * Options for async action execution
 */
interface AsyncActionOptions<T, S> {
  /** The async operation to perform */
  operation: () => Promise<T>;
  /** Called with the result on success */
  onSuccess: (result: T, set: (fn: (state: S) => S) => void) => void;
  /** Optional error message prefix */
  errorPrefix?: string;
  /** Whether to re-throw errors (default: false) */
  rethrow?: boolean;
}

/**
 * Options for retry utility
 */
interface RetryOptions {
  /** Maximum number of retry attempts */
  maxRetries: number;
  /** Delay between retries in milliseconds */
  delayMs: number;
}

/**
 * Execute an action with retry logic.
 * Retries the action if it throws or returns a falsy result.
 *
 * @example
 * ```ts
 * retryAction(
 *   () => {
 *     if (!window.SomeBridge) return false;
 *     window.SomeBridge.doSomething();
 *     return true;
 *   },
 *   { maxRetries: 5, delayMs: 200 }
 * );
 * ```
 */
export function retryAction(
  action: () => boolean | void,
  options: RetryOptions
): void {
  const { maxRetries, delayMs } = options;
  
  const tryAction = (retriesLeft: number) => {
    try {
      const result = action();
      // If action returns explicit false, retry
      if (result === false && retriesLeft > 0) {
        setTimeout(() => tryAction(retriesLeft - 1), delayMs);
      }
    } catch {
      // On error, retry if attempts remain
      if (retriesLeft > 0) {
        setTimeout(() => tryAction(retriesLeft - 1), delayMs);
      }
    }
  };
  
  tryAction(maxRetries);
}

/**
 * Helper function to execute async actions with consistent loading/error state management.
 * Reduces boilerplate in Zustand stores.
 *
 * @example
 * ```ts
 * loadConfig: async () => {
 *   await executeAsync({
 *     operation: () => invoke<AppConfig>('load_config'),
 *     onSuccess: (config, set) => set((state) => ({ ...state, config })),
 *     errorPrefix: 'Failed to load config',
 *   }, set);
 * },
 * ```
 */
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
