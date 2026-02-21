import { useCallback, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export interface SAFPickerResult {
  uri: string;
  name: string;
}

// 声明全局window扩展
declare global {
  interface Window {
    SAFPickerAndroid?: {
      openPicker: (initialUri: string | null, callback: string) => boolean;
    };
  }
}

export function useSAFPicker() {
  const cleanupRef = useRef<(() => void) | null>(null);
  const callbackRef = useRef<((uri: string | null) => void) | null>(null);

  const openPicker = useCallback(async (initialUri?: string): Promise<SAFPickerResult | null> => {
    // 清理之前的会话
    if (cleanupRef.current) {
      cleanupRef.current();
      cleanupRef.current = null;
    }

    // 检测平台
    const isAndroid = typeof navigator !== 'undefined' && 
      /Android/i.test(navigator.userAgent);
    
    const isTauri = typeof window !== 'undefined' && 
      (window as any).__TAURI__ !== undefined;
    
    console.log('[useSAFPicker] Platform detection:', { isAndroid, isTauri, hasBridge: !!window.SAFPickerAndroid });

    // 桌面端：使用Tauri对话框
    if (!isAndroid) {
      try {
        const result = await invoke<string | null>('select_save_directory');
        if (result) {
          return {
            uri: result,
            name: result.split('/').pop() || 'Selected Folder',
          };
        }
        return null;
      } catch (err) {
        console.error('Failed to open directory picker:', err);
        return null;
      }
    }

    // Android端：优先使用JavaScript Bridge
    if (window.SAFPickerAndroid?.openPicker) {
      console.log('[useSAFPicker] Using JavaScript Bridge');
      
      return new Promise((resolve) => {
        // 创建唯一的回调函数名
        const callbackName = `_safPickerCallback_${Date.now()}`;
        
        // 设置回调
        callbackRef.current = (uri: string | null) => {
          console.log('[useSAFPicker] Bridge callback received:', uri);
          
          // 清理
          delete (window as any)[callbackName];
          callbackRef.current = null;
          cleanupRef.current = null;
          
          if (uri) {
            resolve({
              uri,
              name: extractPathName(uri),
            });
          } else {
            resolve(null);
          }
        };
        
        // 将回调注册到window对象，供Android调用
        (window as any)[callbackName] = (uri: string | null) => {
          callbackRef.current?.(uri);
        };
        
        // 调用Android Bridge
        const jsCallback = `${callbackName}`;
        console.log('[useSAFPicker] Calling bridge with callback:', jsCallback);
        
        try {
          const success = window.SAFPickerAndroid!.openPicker(initialUri || null, jsCallback);
          console.log('[useSAFPicker] Bridge call success:', success);
          
          if (!success) {
            resolve(null);
          }
        } catch (err) {
          console.error('[useSAFPicker] Bridge call failed:', err);
          resolve(null);
        }
        
        // 设置清理函数
        cleanupRef.current = () => {
          delete (window as any)[callbackName];
          callbackRef.current = null;
        };
        
        // 60秒超时
        setTimeout(() => {
          if (callbackRef.current) {
            console.log('[useSAFPicker] Timeout');
            callbackRef.current(null);
          }
        }, 60000);
      });
    }

    // 回退：使用Tauri事件机制（备用方案）
    console.log('[useSAFPicker] Falling back to Tauri event mechanism');
    
    return new Promise((resolve) => {
      let unlistenFn: (() => void) | null = null;
      let timeoutId: ReturnType<typeof setTimeout> | null = null;
      let resolved = false;

      const cleanup = () => {
        if (timeoutId) {
          clearTimeout(timeoutId);
          timeoutId = null;
        }
        if (unlistenFn) {
          unlistenFn();
          unlistenFn = null;
        }
        cleanupRef.current = null;
      };

      cleanupRef.current = cleanup;
      
      // 设置监听器
      const setupListener = async () => {
        try {
          unlistenFn = await listen<{ uri: string | null }>('saf-picker-result', (event) => {
            if (resolved) return;
            resolved = true;
            
            cleanup();
            
            if (event.payload.uri) {
              resolve({
                uri: event.payload.uri,
                name: extractPathName(event.payload.uri),
              });
            } else {
              resolve(null);
            }
          });
          
          // 请求打开选择器
          await invoke('request_saf_picker', { initialUri });
        } catch (err) {
          console.error('Failed to setup picker:', err);
          if (!resolved) {
            resolved = true;
            cleanup();
            resolve(null);
          }
        }
      };
      
      setupListener();
      
      // 超时
      timeoutId = setTimeout(() => {
        if (resolved) return;
        resolved = true;
        cleanup();
        resolve(null);
      }, 60000);
    });
  }, []);

  const cleanup = useCallback(() => {
    if (cleanupRef.current) {
      cleanupRef.current();
      cleanupRef.current = null;
    }
  }, []);

  return { openPicker, cleanup };
}

// 辅助函数：从URI提取路径名
function extractPathName(uri: string): string {
  const treeMatch = uri.match(/:([^:]+)$/);
  if (treeMatch) {
    return treeMatch[1];
  }
  
  const segments = uri.split('/');
  return segments[segments.length - 1] || 'Selected Folder';
}
