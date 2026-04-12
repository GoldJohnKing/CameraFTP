/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { FileInfo } from '../types';

/**
 * Generates indices scanning outward from `start`: forward first, then backward.
 * E.g., start=2, length=5 → [3, 4, 1, 0]
 */
function* forwardThenBackwardRange(start: number, length: number): Generator<number> {
  for (let i = start + 1; i < length; i++) yield i;
  for (let i = start - 1; i >= 0; i--) yield i;
}

interface UsePreviewNavigationParams {
  imagePath: string | null;
  onImagePathChange: (path: string | null) => void;
  onBeforeNavigation: () => void;
  onNavigationSettled: () => void;
}

interface UsePreviewNavigationResult {
  currentIndex: number;
  totalFiles: number;
  goToPrevious: () => void;
  goToNext: () => void;
  goToOldest: () => void;
  goToLatest: () => Promise<void>;
}

export function usePreviewNavigation({
  imagePath,
  onImagePathChange,
  onBeforeNavigation,
  onNavigationSettled,
}: UsePreviewNavigationParams): UsePreviewNavigationResult {
  const [currentIndex, setCurrentIndex] = useState(0);
  const [totalFiles, setTotalFiles] = useState(0);

  useEffect(() => {
    const loadFileInfo = async () => {
      try {
        const files = await invoke<FileInfo[]>('get_file_list');
        setTotalFiles(files.length);

        const index = await invoke<number | null>('get_current_file_index');
        setCurrentIndex(index ?? 0);
      } catch {
      }
    };

    void loadFileInfo();
  }, [imagePath]);

  useEffect(() => {
    const unlistenPromise = listen<{ count: number; latestFilename: string | null }>(
      'file-index-changed',
      (event) => {
        setTotalFiles(event.payload.count);
        setCurrentIndex((prev) => {
          if (event.payload.count === 0) return 0;
          return Math.min(prev, event.payload.count - 1);
        });
      },
    );

    return () => {
      unlistenPromise.then((unlisten) => unlisten()).catch(() => {});
    };
  }, []);

  const navigateTo = useCallback(async (targetIndex: number) => {
    if (targetIndex < 0 || targetIndex >= totalFiles) return;

    try {
      const file = await invoke<FileInfo>('navigate_to_file', { index: targetIndex });
      setCurrentIndex(targetIndex);
      onBeforeNavigation();
      onImagePathChange(file.path);
      onNavigationSettled();
    } catch {
      try {
        const files = await invoke<FileInfo[]>('get_file_list');
        setTotalFiles(files.length);

        if (files.length === 0) {
          setCurrentIndex(0);
          onImagePathChange(null);
          return;
        }

        const startIndex = Math.min(targetIndex, files.length - 1);

        // Scan outward from startIndex: first forward, then backward
        for (const candidate of [startIndex, ...forwardThenBackwardRange(startIndex, files.length)]) {
          try {
            const file = await invoke<FileInfo>('navigate_to_file', { index: candidate });
            setCurrentIndex(candidate);
            onBeforeNavigation();
            onImagePathChange(file.path);
            onNavigationSettled();
            return;
          } catch {
            continue;
          }
        }
      } catch {
      }
    }
  }, [onBeforeNavigation, onImagePathChange, onNavigationSettled, totalFiles]);

  const goToPrevious = useCallback(() => {
    if (totalFiles === 0) return;
    void navigateTo(currentIndex + 1);
  }, [currentIndex, navigateTo, totalFiles]);

  const goToNext = useCallback(() => {
    if (totalFiles === 0) return;
    void navigateTo(currentIndex - 1);
  }, [currentIndex, navigateTo, totalFiles]);

  const goToOldest = useCallback(() => {
    if (totalFiles === 0) return;
    void navigateTo(totalFiles - 1);
  }, [totalFiles, navigateTo]);

  const goToLatest = useCallback(async () => {
    if (totalFiles === 0) return;
    await navigateTo(0);
  }, [navigateTo, totalFiles]);

  return {
    currentIndex,
    totalFiles,
    goToPrevious,
    goToNext,
    goToOldest,
    goToLatest,
  };
}
