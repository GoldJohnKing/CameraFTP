/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useState, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';

type PortSyntaxValidationResult =
  | { valid: false; reason: 'empty' | 'invalid_number' | 'out_of_range' }
  | { valid: true; port: number };

interface UsePortCheckResult {
  checkPort: (port: number) => Promise<{ available: boolean }>;
  isChecking: boolean;
}

export function parsePortInput(
  value: string,
  minPort: number,
  maxPort: number,
): PortSyntaxValidationResult {
  if (value.trim() === '') {
    return { valid: false, reason: 'empty' };
  }

  const port = parseInt(value, 10);
  if (isNaN(port) || port < 1 || port > 65535) {
    return { valid: false, reason: 'invalid_number' };
  }

  if (port < minPort || port > maxPort) {
    return { valid: false, reason: 'out_of_range' };
  }

  return { valid: true, port };
}

export function usePortCheck(): UsePortCheckResult {
  const [isChecking, setIsChecking] = useState(false);

  const checkPort = useCallback(async (port: number) => {
    setIsChecking(true);

    try {
      const available = await invoke<boolean>('check_port_available', { port });
      return { available };
    } catch {
      return { available: false };
    } finally {
      setIsChecking(false);
    }
  }, []);

  return { checkPort, isChecking };
}
