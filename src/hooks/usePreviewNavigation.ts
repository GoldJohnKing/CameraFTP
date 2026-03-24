/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { FileInfo } from '../types';

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

        const newIndex = Math.min(targetIndex, files.length - 1);
        setCurrentIndex(newIndex);

        try {
          const file = await invoke<FileInfo>('navigate_to_file', { index: newIndex });
          onImagePathChange(file.path);
          onBeforeNavigation();
          onNavigationSettled();
        } catch {
          if (newIndex < files.length - 1) {
            void navigateTo(newIndex + 1);
          } else if (newIndex > 0) {
            void navigateTo(newIndex - 1);
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

    try {
      const file = await invoke<FileInfo>('navigate_to_file', { index: 0 });
      setCurrentIndex(0);
      onBeforeNavigation();
      setTotalFiles((prev) => Math.max(prev, 1));
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

        if (files.length > 0) {
          try {
            const file = await invoke<FileInfo>('navigate_to_file', { index: 0 });
            setCurrentIndex(0);
            onBeforeNavigation();
            onImagePathChange(file.path);
            onNavigationSettled();
          } catch {
            for (let i = 0; i < files.length; i++) {
              try {
                const file = await invoke<FileInfo>('navigate_to_file', { index: i });
                setCurrentIndex(i);
                onBeforeNavigation();
                onImagePathChange(file.path);
                onNavigationSettled();
                break;
              } catch {
                continue;
              }
            }
          }
        }
      } catch {
      }
    }
  }, [onBeforeNavigation, onImagePathChange, onNavigationSettled, totalFiles]);

  return {
    currentIndex,
    totalFiles,
    goToPrevious,
    goToNext,
    goToOldest,
    goToLatest,
  };
}
