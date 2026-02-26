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
    set((state) => ({ ...state, isLoading: false }));
    return result;
  } catch (err: unknown) {
    let errorMessage = formatError(err);
    if (errorPrefix) {
      errorMessage = `${errorPrefix}: ${errorMessage}`;
    }
    set((state) => ({ ...state, error: errorMessage, isLoading: false }));
    if (rethrow) {
      throw err;
    }
    return undefined;
  }
}
