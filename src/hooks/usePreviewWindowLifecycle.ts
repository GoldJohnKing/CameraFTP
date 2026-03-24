/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { PREVIEW_NAVIGATE_EVENT } from './preview-window-events';

interface PreviewEvent {
  file_path: string;
  bring_to_front: boolean;
}

interface PreviewWindowState {
  isOpen: boolean;
  currentImage: string | null;
  autoBringToFront: boolean;
}

const initialPreviewWindowState: PreviewWindowState = {
  isOpen: false,
  currentImage: null,
  autoBringToFront: false,
};

export function usePreviewWindowLifecycle(): PreviewWindowState {
  const [state, setState] = useState<PreviewWindowState>(initialPreviewWindowState);

  useEffect(() => {
    const loadPlatform = async () => {
      try {
        const platformValue = await invoke<string>('get_platform');
        document.documentElement.className = `platform-${platformValue}`;
      } catch {
      }
    };

    void loadPlatform();
  }, []);

  useEffect(() => {
    let isMounted = true;
    let unlisten: (() => void) | null = null;

    const setupListener = async () => {
      const listener = await listen<PreviewEvent>('preview-image', (event) => {
        const { file_path, bring_to_front } = event.payload;

        setState((prev) => ({
          ...prev,
          isOpen: true,
          currentImage: file_path,
          autoBringToFront: bring_to_front,
        }));
      });

      if (!isMounted) {
        listener();
        return;
      }

      unlisten = listener;
    };

    void setupListener();

    return () => {
      isMounted = false;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    const handleNavigate = (e: Event) => {
      const customEvent = e as CustomEvent<string | null>;
      setState((prev) => ({
        ...prev,
        currentImage: customEvent.detail,
      }));
    };

    window.addEventListener(PREVIEW_NAVIGATE_EVENT, handleNavigate);
    return () => window.removeEventListener(PREVIEW_NAVIGATE_EVENT, handleNavigate);
  }, []);

  return state;
}
