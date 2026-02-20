import { useEffect, useRef } from 'react';
import { listen, type UnlistenFn, type EventCallback, type EventName } from '@tauri-apps/api/event';

export interface ListenerConfig<T = unknown> {
  event: EventName;
  handler: EventCallback<T>;
}

export function useTauriListeners(configs: ListenerConfig[]) {
  const unlistenersRef = useRef<UnlistenFn[]>([]);

  useEffect(() => {
    let isMounted = true;

    const setupListeners = async () => {
      const unlisteners: UnlistenFn[] = [];
      
      for (const config of configs) {
        try {
          const unlisten = await listen(config.event, config.handler);
          if (isMounted) {
            unlisteners.push(unlisten);
          } else {
            unlisten();
          }
        } catch (err) {
          console.error(`Failed to listen to ${String(config.event)}:`, err);
        }
      }
      
      if (isMounted) {
        unlistenersRef.current = unlisteners;
      }
    };

    setupListeners();

    return () => {
      isMounted = false;
      unlistenersRef.current.forEach(unlisten => {
        try {
          unlisten();
        } catch (err) {
          console.error('Error unlistening:', err);
        }
      });
      unlistenersRef.current = [];
    };
  }, [configs]);
}
