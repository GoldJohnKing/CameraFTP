import { useCallback, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export interface SAFPickerResult {
  uri: string;
  name: string;
}

export function useSAFPicker() {
  const cleanupRef = useRef<(() => void) | null>(null);

  const openPicker = useCallback(async (initialUri?: string): Promise<SAFPickerResult | null> => {
    // Clean up any previous picker session
    if (cleanupRef.current) {
      cleanupRef.current();
      cleanupRef.current = null;
    }

    // Check if we're on Android
    const isAndroid = typeof navigator !== 'undefined' && 
      /Android/i.test(navigator.userAgent);
    
    if (!isAndroid) {
      // Desktop: use Tauri's dialog
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

    // Android: Use event-based SAF picker
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

      // Store cleanup for unmount handling
      cleanupRef.current = cleanup;
      
      // Set up listener for picker result
      const setupListener = async () => {
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
      };
      
      setupListener();
      
      // Request to open picker
      invoke('request_saf_picker', { initialUri }).catch((err) => {
        console.error('Failed to request SAF picker:', err);
        if (resolved) return;
        resolved = true;
        cleanup();
        resolve(null);
      });
      
      // Timeout after 60 seconds
      timeoutId = setTimeout(() => {
        if (resolved) return;
        resolved = true;
        cleanup();
        resolve(null);
      }, 60000);
    });
  }, []);

  // Cleanup function for unmount
  const cleanup = useCallback(() => {
    if (cleanupRef.current) {
      cleanupRef.current();
      cleanupRef.current = null;
    }
  }, []);

  return { openPicker, cleanup };
}

// Helper function to extract path name from content:// URI
function extractPathName(uri: string): string {
  // Pattern 1: content://.../tree/primary:DCIM/Camera
  // Pattern 2: content://.../document/primary:DCIM/Camera
  
  const treeMatch = uri.match(/:([^:]+)$/);
  if (treeMatch) {
    return treeMatch[1];
  }
  
  // Fallback: use last path segment
  const segments = uri.split('/');
  return segments[segments.length - 1] || 'Selected Folder';
}
