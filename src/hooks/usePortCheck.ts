import { useState, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { validatePort } from '../utils/validation';

interface UsePortCheckResult {
  checkPort: (value: string) => Promise<{ valid: boolean; available: boolean }>;
  isChecking: boolean;
  portError: string | null;
  clearError: () => void;
}

export function usePortCheck(): UsePortCheckResult {
  const [isChecking, setIsChecking] = useState(false);
  const [portError, setPortError] = useState<string | null>(null);

  const checkPort = useCallback(async (value: string) => {
    const port = validatePort(value);
    if (port === null) {
      setPortError('Port must be between 1 and 65535');
      return { valid: false, available: false };
    }

    setIsChecking(true);
    setPortError(null);

    try {
      const available = await invoke<boolean>('check_port_available', { port });
      if (!available) {
        setPortError('Port is already in use');
      }
      return { valid: true, available };
    } catch (e) {
      setPortError('Failed to check port availability');
      return { valid: true, available: false };
    } finally {
      setIsChecking(false);
    }
  }, []);

  const clearError = useCallback(() => {
    setPortError(null);
  }, []);

  return { checkPort, isChecking, portError, clearError };
}
