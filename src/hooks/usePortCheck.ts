/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useState, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { validatePort } from '../utils/validation';

interface UsePortCheckResult {
  checkPort: (value: string) => Promise<{ valid: boolean; available: boolean }>;
  isChecking: boolean;
}

export function usePortCheck(): UsePortCheckResult {
  const [isChecking, setIsChecking] = useState(false);

  const checkPort = useCallback(async (value: string) => {
    const port = validatePort(value);
    if (port === null) {
      return { valid: false, available: false };
    }

    setIsChecking(true);

    try {
      const available = await invoke<boolean>('check_port_available', { port });
      return { valid: true, available };
    } catch {
      return { valid: true, available: false };
    } finally {
      setIsChecking(false);
    }
  }, []);

  return { checkPort, isChecking };
}
