/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { AlertCircle } from 'lucide-react';

interface ErrorMessageProps {
  message: string | null;
}

export function ErrorMessage({ message }: ErrorMessageProps) {
  if (!message) return null;

  return (
    <p className="mt-3 text-sm text-red-600 text-center flex items-center justify-center gap-1">
      <AlertCircle className="w-4 h-4" />
      {message}
    </p>
  );
}
