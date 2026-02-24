/**
 * 通用错误处理工具
 * 提供统一的错误格式化、错误处理和错误回调创建功能
 */

/**
 * 将未知错误格式化为字符串
 */
export function formatError(err: unknown): string {
  if (err instanceof Error) {
    return err.message;
  }
  if (typeof err === 'string') {
    return err;
  }
  if (err === null || err === undefined) {
    return 'Unknown error';
  }
  try {
    return JSON.stringify(err);
  } catch {
    return String(err);
  }
}

/**
 * 创建错误处理器，用于 store 中的 setError 函数
 */
export function createErrorHandler(setError: (error: string | null) => void) {
  return (err: unknown): never => {
    const message = formatError(err);
    setError(message);
    throw err;
  };
}

/**
 * 带错误处理的异步函数包装器
 * 自动捕获错误并设置错误状态
 */
export async function withErrorHandling<T>(
  fn: () => Promise<T>,
  setError: (error: string | null) => void,
  defaultValue?: T
): Promise<T | undefined> {
  try {
    setError(null);
    return await fn();
  } catch (err) {
    const message = formatError(err);
    setError(message);
    console.error('Operation failed:', err);
    return defaultValue;
  }
}

/**
 * 创建带取消标记的异步操作
 * 用于防止组件卸载后的状态更新
 */
export function createCancellableOperation() {
  let isCancelled = false;

  return {
    get isCancelled() {
      return isCancelled;
    },
    cancel() {
      isCancelled = true;
    },
    /**
     * 执行操作，如果已取消则不执行回调
     */
    execute<T>(
      operation: () => Promise<T>,
      onSuccess?: (result: T) => void,
      onError?: (err: unknown) => void
    ): Promise<void> {
      return operation()
        .then((result) => {
          if (!isCancelled && onSuccess) {
            onSuccess(result);
          }
        })
        .catch((err) => {
          if (!isCancelled && onError) {
            onError(err);
          } else if (!isCancelled) {
            console.error('Operation failed:', err);
          }
        });
    },
  };
}
