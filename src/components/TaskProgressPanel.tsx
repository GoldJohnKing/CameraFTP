/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useEffect, useRef } from 'react';
import { useAiEditProgress, dismissDone as dismissAiEditDone, cancelAiEdit } from '../hooks/useAiEditProgress';
import {
  useColorGradingProgress,
  dismissColorGradingDone,
  cancelColorGrading,
} from '../hooks/useColorGradingProgress';

interface TaskProgressPanelProps {
  position: 'absolute' | 'fixed';
}

const AUTO_DISMISS_DELAY_MS = 3000;

export function TaskProgressPanel({ position }: TaskProgressPanelProps) {
  const aiEdit = useAiEditProgress();
  const colorGrading = useColorGradingProgress();
  const dismissTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const aiEditVisible = aiEdit.isEditing || aiEdit.isDone;
  const cgVisible = colorGrading.isProcessing || colorGrading.isDone;
  const hasAnyTask = aiEditVisible || cgVisible;

  const allDone = (aiEditVisible ? aiEdit.isDone : true)
    && (cgVisible ? colorGrading.isDone : true)
    && hasAnyTask;

  useEffect(() => {
    if (dismissTimerRef.current) {
      clearTimeout(dismissTimerRef.current);
      dismissTimerRef.current = null;
    }

    if (allDone) {
      dismissTimerRef.current = setTimeout(() => {
        dismissAiEditDone();
        dismissColorGradingDone();
      }, AUTO_DISMISS_DELAY_MS);
    }

    return () => {
      if (dismissTimerRef.current) {
        clearTimeout(dismissTimerRef.current);
        dismissTimerRef.current = null;
      }
    };
  }, [allDone]);

  if (!hasAnyTask) return null;

  const containerClass = position === 'fixed' ? 'fixed z-50' : 'absolute z-10';
  const containerStyle: React.CSSProperties = position === 'fixed'
    ? { left: '12px', bottom: '5rem' }
    : { left: '12px', bottom: '76px' };

  const handleCancelAiEdit = () => { void cancelAiEdit(); };
  const handleCancelColorGrading = () => { void cancelColorGrading(); };
  const handleCancelAll = () => {
    if (aiEditVisible && !aiEdit.isDone) void cancelAiEdit();
    if (cgVisible && !colorGrading.isDone) void cancelColorGrading();
  };

  return (
    <div className={`${containerClass} animate-slide-up`} style={containerStyle}>
      <div className="bg-gray-950/80 backdrop-blur-md border border-white/10 rounded-lg overflow-hidden min-w-[180px]">
        {/* Header */}
        <div className="px-3 py-1 text-center text-[11px] text-white/50 border-b border-white/10">
          后台任务
        </div>

        {/* AI Edit row */}
        {aiEditVisible && (
          <div className="flex items-center px-3 py-1.5 gap-1">
            <span className="text-blue-400 text-xs shrink-0">AI修图：</span>
            <span className="text-white/70 text-xs tabular-nums">
              {aiEdit.isDone ? aiEdit.total : aiEdit.current} / {aiEdit.total}
            </span>
            {aiEdit.failedCount > 0 && (
              <span className="text-red-400 text-xs tabular-nums">
                (失败 {aiEdit.failedCount})
              </span>
            )}
            {!aiEdit.isDone && (
              <button
                onClick={handleCancelAiEdit}
                className="ml-auto text-white/40 hover:text-white text-xs p-0.5 shrink-0"
                aria-label="取消AI修图"
              >
                ×
              </button>
            )}
          </div>
        )}

        {/* Color Grading row */}
        {cgVisible && (
          <div className="flex items-center px-3 py-1.5 gap-1">
            <span className="text-violet-400 text-xs shrink-0">调色：</span>
            <span className="text-white/70 text-xs tabular-nums">
              {colorGrading.isDone ? colorGrading.total : colorGrading.current} / {colorGrading.total}
            </span>
            {colorGrading.failedCount > 0 && (
              <span className="text-red-400 text-xs tabular-nums">
                (失败 {colorGrading.failedCount})
              </span>
            )}
            {!colorGrading.isDone && (
              <button
                onClick={handleCancelColorGrading}
                className="ml-auto text-white/40 hover:text-white text-xs p-0.5 shrink-0"
                aria-label="取消调色"
              >
                ×
              </button>
            )}
          </div>
        )}

        {/* Footer */}
        <div className="border-t border-white/10 px-3 py-1 text-center">
          {allDone ? (
            <span className="text-green-400 text-[11px]">已完成</span>
          ) : (
            <button
              onClick={handleCancelAll}
              className="text-white/40 hover:text-white text-[11px]"
            >
              全部取消
            </button>
          )}
        </div>
      </div>
    </div>
  );
}
