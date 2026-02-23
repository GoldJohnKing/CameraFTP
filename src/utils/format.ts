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

/**
 * 将字节数格式化为最合适的单位
 * @param bytes 字节数
 * @returns 格式化后的字符串，如 "1.2 GB"、"512.0 MB"
 */
export function formatBytesAuto(bytes: number): string {
  if (bytes === 0) return '0 B';
  
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  const k = 1024;
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  const index = Math.min(i, units.length - 1);
  
  return `${(bytes / Math.pow(k, index)).toFixed(1)} ${units[index]}`;
}
