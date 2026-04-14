/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { ReactNode } from 'react';
import { X } from 'lucide-react';

interface DialogProps {
  isOpen: boolean;
  onClose: () => void;
  title: string;
  subtitle?: string;
  icon?: ReactNode;
  children: ReactNode;
  footer?: ReactNode;
  maxWidth?: string;
  maxHeight?: string;
  zIndex?: string;
  overlayClassName?: string;
  contentClassName?: string;
  'data-testid'?: string;
}

export function Dialog({
  isOpen,
  onClose,
  title,
  subtitle,
  icon,
  children,
  footer,
  maxWidth = 'max-w-md',
  maxHeight,
  zIndex = 'z-50',
  overlayClassName = 'bg-black/50',
  contentClassName = 'p-4',
  'data-testid': testId,
}: DialogProps) {
  if (!isOpen) return null;

  return (
    <div className={`fixed inset-0 ${overlayClassName} flex items-center justify-center ${zIndex} p-4`} data-testid={testId}>
      <div className={`bg-white rounded-xl ${maxWidth} w-full shadow-2xl flex flex-col ${maxHeight ?? ''}`}>
        {/* Header */}
        <div className="flex items-center justify-between p-4 border-b shrink-0">
          <div className="flex items-center gap-3">
            {icon}
            <div>
              <h2 className="text-lg font-semibold text-gray-900">{title}</h2>
              {subtitle && (
                <p className="text-sm text-gray-500 mt-0.5">{subtitle}</p>
              )}
            </div>
          </div>
          <button
            onClick={onClose}
            className="p-2 text-gray-400 hover:text-gray-600 hover:bg-gray-100 rounded-lg transition-colors"
            aria-label="关闭"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Content */}
        <div className={`overflow-y-auto ${contentClassName}`}>{children}</div>

        {/* Footer */}
        {footer && (
          <div className="border-t p-4 flex justify-end shrink-0">
            {footer}
          </div>
        )}
      </div>
    </div>
  );
}
