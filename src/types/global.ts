/**
 * 全局类型定义
 * 包含窗口扩展类型和 JS Bridge 类型
 */

import { Event } from '@tauri-apps/api/event';

// ===== Android JS Bridge 类型 =====

/**
 * Android 文件上传回调接口
 * 由 Android WebView 注入
 */
export interface FileUploadAndroid {
  /**
   * 文件上传完成回调
   * @param path 文件路径
   * @param size 文件大小（字节）
   */
  onFileUploaded: (path: string, size: number) => void;
}

/**
 * Android SAF 选择器接口
 * 由 Android WebView 注入
 */
export interface SAFPickerAndroid {
  /**
   * 打开"所有文件访问权限"设置页面
   */
  openAllFilesAccessSettings: () => void;
}

// ===== 全局窗口扩展 =====

declare global {
  interface Window {
    /**
     * Android 文件上传 JS Bridge
     */
    FileUploadAndroid?: FileUploadAndroid;
    
    /**
     * Android SAF 选择器 JS Bridge
     */
    SAFPickerAndroid?: SAFPickerAndroid;
  }
}

// ===== 类型守卫函数 =====

/**
 * 检查 Android 文件上传桥接是否可用
 */
export function isAndroidFileUploadAvailable(): boolean {
  return typeof window !== 'undefined' && 
         !!window.FileUploadAndroid && 
         typeof window.FileUploadAndroid.onFileUploaded === 'function';
}

/**
 * 检查 Android SAF 选择器是否可用
 */
export function isSAFPickerAvailable(): boolean {
  return typeof window !== 'undefined' && 
         !!window.SAFPickerAndroid && 
         typeof window.SAFPickerAndroid.openAllFilesAccessSettings === 'function';
}

/**
 * 触发媒体扫描（仅在 Android 平台有效）
 */
export function triggerMediaScan(path: string, size: number): void {
  if (isAndroidFileUploadAvailable()) {
    try {
      window.FileUploadAndroid!.onFileUploaded(path, size);
    } catch (err) {
      console.error('Failed to trigger media scan:', err);
    }
  }
}

/**
 * 打开 Android 存储权限设置（仅在 Android 平台有效）
 */
export function openStorageSettings(): void {
  if (isSAFPickerAvailable()) {
    try {
      window.SAFPickerAndroid!.openAllFilesAccessSettings();
    } catch (err) {
      console.error('Failed to open storage settings:', err);
    }
  }
}

// ===== 事件类型定义 =====

/**
 * 服务器启动事件载荷
 */
export interface ServerStartedPayload {
  ip: string;
  port: number;
}

/**
 * 文件上传事件载荷
 */
export interface FileUploadedPayload {
  path: string;
  size: number;
}

/**
 * 事件处理器类型
 */
export type EventHandler<T = unknown> = (event: Event<T>) => void;
