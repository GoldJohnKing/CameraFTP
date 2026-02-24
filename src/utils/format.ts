// src/utils/format.ts

/**
 * 将字节数格式化为可读的 MB 字符串
 * @param bytes 字节数
 * @returns 格式化后的字符串，如 "12.5 MB"
 */
export function formatBytes(bytes: number): string {
  if (bytes === 0) return '0 MB';
  const mb = bytes / (1024 * 1024);
  return `${mb.toFixed(1)} MB`;
}
