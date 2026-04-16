/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useAiEditProgress, dismissDone, cancelAiEdit } from '../hooks/useAiEditProgress';
import { X } from 'lucide-react';

interface AiEditProgressBarProps {
  position: 'absolute' | 'fixed';
}

export function AiEditProgressBar({ position }: AiEditProgressBarProps) {
  const { isEditing, isDone, current, total, failedCount } = useAiEditProgress();

  if (!isEditing && !isDone) return null;

  const hasFailures = failedCount > 0;
  const progressPercent = total > 0 ? (current / total) * 100 : 0;

  const containerClass = position === 'fixed'
    ? 'fixed bottom-20 left-4 right-4 z-50'
    : 'absolute left-4 right-4 z-10';
  const bottomStyle = position === 'absolute' ? { bottom: '76px' } : undefined;

  const handleButtonClick = () => {
    if (isDone) {
      dismissDone();
    } else if (isEditing) {
      void cancelAiEdit();
    }
  };

  return (
    <div
      className={`${containerClass} transition-all duration-300 ease-in-out`}
      style={bottomStyle}
    >
      <div
        className={`
          rounded-xl backdrop-blur-sm px-4 py-3 flex items-center gap-3
          transition-colors duration-300
          ${isDone && hasFailures ? 'bg-red-500/80' : 'bg-black/70'}
        `}
      >
        {!isDone && (
          <div className="flex-1">
            <div className="h-1.5 bg-white/20 rounded-full overflow-hidden">
              <div
                className="h-full bg-gradient-to-r from-blue-500 to-blue-400 rounded-full transition-all duration-500 ease-out shimmer"
                style={{ width: `${progressPercent}%` }}
              />
            </div>
          </div>
        )}

        <span className="text-white text-sm font-medium whitespace-nowrap">
          {isDone
            ? `修图完成，${failedCount}张失败`
            : hasFailures
              ? `第${current}张/共${total}张 (失败${failedCount}张)`
              : `第${current}张/共${total}张`}
        </span>

        <button
          onClick={handleButtonClick}
          className="p-1 text-white/60 hover:text-white transition-colors rounded-full hover:bg-white/10"
        >
          {isDone ? (
            <X className="w-4 h-4" />
          ) : (
            <span className="text-xs font-medium px-1">取消</span>
          )}
        </button>
      </div>

      <style>{`
        @keyframes shimmer {
          0% { background-position: -200% 0; }
          100% { background-position: 200% 0; }
        }
        .shimmer {
          background-size: 200% 100%;
          animation: shimmer 2s linear infinite;
        }
      `}</style>
    </div>
  );
}
