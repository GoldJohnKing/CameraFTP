/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { createEventManager } from '../utils/events';

interface UseQuitFlowResult {
  showQuitDialog: boolean;
  closeQuitDialog: () => void;
  handleQuitConfirm: (quit: boolean) => Promise<void>;
}

interface UseQuitFlowOptions {
  enabled?: boolean;
}

export function useQuitFlow({ enabled = true }: UseQuitFlowOptions = {}): UseQuitFlowResult {
  const [showQuitDialog, setShowQuitDialog] = useState(false);

  useEffect(() => {
    if (!enabled) {
      setShowQuitDialog(false);
      return;
    }

    const eventManager = createEventManager();
    let isDisposed = false;
    let setupResolved = false;

    const setup = async () => {
      await eventManager.on('window-close-requested', async () => {
        try {
          await invoke('show_main_window');
        } catch {
          // Ignore window display errors
        }
        setShowQuitDialog(true);
      });

      setupResolved = true;

      if (isDisposed) {
        eventManager.cleanup();
      }
    };

    setup();

    return () => {
      isDisposed = true;

      if (setupResolved) {
        eventManager.cleanup();
      }
    };
  }, [enabled]);

  const closeQuitDialog = useCallback(() => {
    setShowQuitDialog(false);
  }, []);

  const handleQuitConfirm = useCallback(async (quit: boolean) => {
    if (quit) {
      await invoke('quit_application');
      return;
    }

    setShowQuitDialog(false);
    try {
      await invoke('hide_main_window');
    } catch (err) {
      console.warn('[useQuitFlow] Failed to hide window:', err);
    }
  }, []);

  return {
    showQuitDialog,
    closeQuitDialog,
    handleQuitConfirm,
  };
}
