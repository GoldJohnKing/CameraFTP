/**
 * 通用错误处理工具
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
