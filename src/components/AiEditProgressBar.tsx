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
  const successCount = total - failedCount;
  const progressPercent = total > 0 ? (current / total) * 100 : 0;

  const containerClass = position === 'fixed'
    ? 'fixed z-50'
    : 'absolute z-10';
  const containerStyle: React.CSSProperties = position === 'fixed'
    ? { left: '16.67%', right: '16.67%', bottom: '5rem' }
    : { left: '16.67%', right: '16.67%', bottom: '76px' };

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
      style={containerStyle}
    >
      <div
        className={`
          relative overflow-hidden rounded-xl backdrop-blur-md
          border transition-colors duration-500
          ${isDone && hasFailures
            ? 'bg-red-950/70 border-red-500/20'
            : isDone
              ? 'bg-green-950/75 border-green-500/20'
              : 'bg-gray-950/75 border-white/10'}
        `}
        role="progressbar"
        aria-valuenow={current}
        aria-valuemin={0}
        aria-valuemax={total}
        aria-label={`AI修图进度: 第${current}张/共${total}张`}
      >
        {/* Progress fill — flowing stripe animation */}
        {!isDone && (
          <div
            className="absolute inset-0 transition-all duration-700 ease-out ai-edit-progress-fill"
            style={{ width: `${Math.max(progressPercent, 3)}%` }}
          />
        )}
        {isDone && hasFailures && (
          <div className="absolute inset-0 bg-red-500/20" />
        )}
        {isDone && !hasFailures && (
          <div className="absolute inset-0 ai-edit-progress-fill-success" />
        )}

        {/* Bottom progress edge line — flowing highlight */}
        <div className="absolute bottom-0 left-0 right-0 h-[2px] bg-white/5">
          {!isDone && (
            <div
              className="h-full transition-all duration-700 ease-out ai-edit-progress-edge"
              style={{ width: `${Math.max(progressPercent, 3)}%` }}
            />
          )}
          {isDone && hasFailures && (
            <div className="h-full w-full bg-red-400/60" />
          )}
          {isDone && !hasFailures && (
            <div className="h-full w-full ai-edit-progress-edge-success" />
          )}
        </div>

        {/* Content row */}
        <div className="relative flex items-center justify-between px-3 py-1.5">
          <div className="flex items-center gap-1.5 min-w-0">
            {!isDone && (
              <span className="text-blue-400 text-xs font-medium whitespace-nowrap">
                AI修图中...
              </span>
            )}
            {isDone && !hasFailures && (
              <>
                <span className="text-white text-xs font-medium whitespace-nowrap">
                  修图完成
                </span>
                <span className="text-white/70 text-xs tabular-nums whitespace-nowrap">
                  共{total}张
                </span>
              </>
            )}
            {isDone && hasFailures && (
              <>
                <span className="text-white text-xs font-medium whitespace-nowrap">
                  修图完成
                </span>
                <span className="text-white/70 text-xs tabular-nums whitespace-nowrap">
                  成功{successCount}张
                </span>
                <span className="text-red-400 text-xs tabular-nums whitespace-nowrap">
                  失败{failedCount}张
                </span>
              </>
            )}

            {!isDone && (
              <span className="text-white/70 text-xs tabular-nums whitespace-nowrap">
                第{current}张/共{total}张
              </span>
            )}

            {!isDone && hasFailures && (
              <span className="text-red-400 text-xs whitespace-nowrap">
                失败{failedCount}张
              </span>
            )}
          </div>

          <button
            onClick={handleButtonClick}
            className="ml-1 p-0.5 text-white/50 hover:text-white rounded-lg
                       hover:bg-white/10 transition-colors shrink-0
                       flex items-center justify-center"
          >
            {isDone ? (
              <X className="w-3.5 h-3.5" />
            ) : (
              <span className="text-[11px] font-medium">取消</span>
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

        @keyframes highlight-sweep {
          0% { background-position: -50% 0; }
          100% { background-position: 200% 0; }
        }
        .ai-edit-progress-fill {
          background:
            linear-gradient(
              90deg,
              transparent 0%,
              rgba(147, 197, 253, 0.3) 50%,
              transparent 100%
            );
          background-color: rgba(59, 130, 246, 0.13);
          background-size: 40% 100%;
          background-repeat: no-repeat;
          animation: highlight-sweep 2s ease-in-out infinite;
        }

        @keyframes edge-highlight-sweep {
          0% { background-position: -50% 0; }
          100% { background-position: 200% 0; }
        }
        .ai-edit-progress-edge {
          background:
            linear-gradient(
              90deg,
              transparent 0%,
              rgba(191, 219, 254, 0.8) 50%,
              transparent 100%
            );
          background-color: rgba(96, 165, 250, 0.5);
          background-size: 40% 100%;
          background-repeat: no-repeat;
          animation: edge-highlight-sweep 2s ease-in-out infinite;
        }

        @media (prefers-reduced-motion: reduce) {
          .animate-slide-up {
            animation: none;
            opacity: 1;
          }
          .ai-edit-progress-fill {
            animation: none;
            background: rgba(59, 130, 246, 0.13);
          }
          .ai-edit-progress-edge {
            animation: none;
            background: rgba(96, 165, 250, 0.5);
          }
        }

        .ai-edit-progress-fill-success {
          background:
            linear-gradient(
              90deg,
              transparent 0%,
              rgba(134, 239, 172, 0.3) 50%,
              transparent 100%
            );
          background-color: rgba(34, 197, 94, 0.13);
          background-size: 40% 100%;
          background-repeat: no-repeat;
          background-position: 50% 0;
        }

        .ai-edit-progress-edge-success {
          background:
            linear-gradient(
              90deg,
              transparent 0%,
              rgba(187, 247, 208, 0.8) 50%,
              transparent 100%
            );
          background-color: rgba(74, 222, 128, 0.5);
          background-size: 40% 100%;
          background-repeat: no-repeat;
          background-position: 50% 0;
        }
      `}</style>
    </div>
  );
}
