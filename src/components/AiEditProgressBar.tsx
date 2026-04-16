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
      className={`${containerClass} animate-slide-up`}
      style={bottomStyle}
    >
      <div
        className={`
          relative overflow-hidden rounded-xl backdrop-blur-md
          border transition-colors duration-500
          ${isDone && hasFailures
            ? 'bg-red-950/70 border-red-500/20'
            : 'bg-gray-950/75 border-white/10'}
        `}
        role="progressbar"
        aria-valuenow={current}
        aria-valuemin={0}
        aria-valuemax={total}
        aria-label={`AI修图进度: 第${current}张/共${total}张`}
      >
        {/* Progress fill — animated background tint */}
        {!isDone && (
          <div
            className="absolute inset-0 bg-blue-500/15 transition-all duration-700 ease-out ai-edit-progress-fill"
            style={{ width: `${Math.max(progressPercent, 3)}%` }}
          />
        )}
        {isDone && hasFailures && (
          <div className="absolute inset-0 bg-red-500/20" />
        )}

        {/* Bottom progress edge line */}
        <div className="absolute bottom-0 left-0 right-0 h-[2px] bg-white/5">
          {!isDone && (
            <div
              className="h-full bg-blue-400/60 transition-all duration-700 ease-out"
              style={{ width: `${Math.max(progressPercent, 3)}%` }}
            />
          )}
          {isDone && hasFailures && (
            <div className="h-full w-full bg-red-400/60" />
          )}
        </div>

        {/* Content row */}
        <div className="relative flex items-center justify-between px-4 py-2.5">
          <div className="flex items-center gap-2 min-w-0">
            {!isDone && (
              <span className="text-blue-400 text-sm font-medium whitespace-nowrap">
                AI修图中...
              </span>
            )}
            {isDone && (
              <span className="text-white text-sm font-medium whitespace-nowrap">
                修图完成
              </span>
            )}

            <span className="text-white/70 text-sm tabular-nums whitespace-nowrap">
              第{current}张/共{total}张
            </span>

            {hasFailures && (
              <span className="text-red-400 text-sm whitespace-nowrap">
                失败{failedCount}张
              </span>
            )}
          </div>

          <button
            onClick={handleButtonClick}
            className="ml-2 p-2 text-white/50 hover:text-white rounded-lg
                       hover:bg-white/10 transition-colors shrink-0
                       min-w-[44px] min-h-[44px] flex items-center justify-center"
          >
            {isDone ? (
              <X className="w-4 h-4" />
            ) : (
              <span className="text-xs font-medium">取消</span>
            )}
          </button>
        </div>
      </div>

      <style>{`
        @keyframes slide-up {
          from {
            opacity: 0;
            transform: translateY(12px);
          }
          to {
            opacity: 1;
            transform: translateY(0);
          }
        }
        .animate-slide-up {
          animation: slide-up 0.35s cubic-bezier(0.16, 1, 0.3, 1) forwards;
        }

        @keyframes progress-pulse {
          0%, 100% { opacity: 0.6; }
          50% { opacity: 1; }
        }
        .ai-edit-progress-fill {
          background: linear-gradient(
            90deg,
            rgba(59, 130, 246, 0.12) 0%,
            rgba(59, 130, 246, 0.25) 50%,
            rgba(59, 130, 246, 0.12) 100%
          );
          background-size: 200% 100%;
          animation: progress-pulse 2.5s ease-in-out infinite;
        }

        @media (prefers-reduced-motion: reduce) {
          .animate-slide-up {
            animation: none;
            opacity: 1;
          }
          .ai-edit-progress-fill {
            animation: none;
            background: rgba(59, 130, 246, 0.18);
          }
        }
      `}</style>
    </div>
  );
}
