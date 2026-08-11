/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useEffect, useRef } from 'react';
import { Calendar, Check, Loader2 } from 'lucide-react';
import { Dialog } from './ui/Dialog';
import { formatDateTitle } from '../utils/date-format';

/** One selectable calendar day in the date-jump picker. */
export interface GalleryDateOption {
  /** Stable calendar-day key, e.g. "2026-7-18" (matches toDateKey). */
  key: string;
  /** Representative epoch-ms of the day (its newest photo). */
  ms: number;
  /** Number of photos captured on that day. */
  count: number;
}

interface DateJumpDialogProps {
  isOpen: boolean;
  /** Unique dates, newest-first (matching the gallery sort order). */
  dates: GalleryDateOption[];
  /** Currently active date key (the first visible photo's day). */
  selectedKey: string | null;
  /** While remaining pages are being loaded to complete the date list. */
  isLoading?: boolean;
  onSelect: (key: string) => void;
  onClose: () => void;
}

export function DateJumpDialog({
  isOpen,
  dates,
  selectedKey,
  isLoading = false,
  onSelect,
  onClose,
}: DateJumpDialogProps) {
  const selectedRef = useRef<HTMLButtonElement>(null);

  // Center the active date when the dialog opens so the user sees where they are.
  useEffect(() => {
    if (!isOpen) return;
    const id = window.setTimeout(() => {
      if (typeof selectedRef.current?.scrollIntoView === 'function') {
        selectedRef.current.scrollIntoView({ block: 'center' });
      }
    }, 0);
    return () => window.clearTimeout(id);
  }, [isOpen]);

  return (
    <Dialog
      isOpen={isOpen}
      onClose={onClose}
      title="跳转到日期"
      icon={<Calendar className="w-5 h-5 text-blue-500" />}
      data-testid="date-jump-dialog"
      contentClassName="p-0"
      maxHeight="max-h-[70vh]"
    >
      {isLoading ? (
        <div className="flex items-center justify-center gap-2 py-16 text-gray-500">
          <Loader2 className="w-6 h-6 animate-spin" />
          <span>加载日期…</span>
        </div>
      ) : dates.length === 0 ? (
        <div className="flex items-center justify-center py-16 text-gray-500">暂无日期</div>
      ) : (
        dates.map((d) => {
          const active = d.key === selectedKey;
          return (
            <button
              key={d.key}
              type="button"
              ref={active ? selectedRef : undefined}
              onClick={() => onSelect(d.key)}
              data-date-key={d.key}
              className={`flex w-full items-center justify-between px-4 py-3 text-left transition-colors duration-150 hover:bg-gray-50 active:bg-blue-100 ${
                active ? 'bg-blue-50' : ''
              }`}
            >
              <span className={`truncate text-sm ${active ? 'font-medium text-blue-600' : 'text-gray-800'}`}>
                {formatDateTitle(d.ms)}
              </span>
              <span className="flex shrink-0 items-center gap-2">
                <span className="text-xs text-gray-400">{d.count} 张</span>
                {active && <Check className="h-4 w-4 text-blue-500" />}
              </span>
            </button>
          );
        })
      )}
    </Dialog>
  );
}
