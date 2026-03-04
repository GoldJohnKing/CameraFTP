import { formatError } from './error';

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
