/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useCallback, useEffect, useRef, useState, type RefObject, type TouchEvent } from 'react';
import { toast } from 'sonner';
import { buildDeleteFailureMessage } from '../utils/gallery-delete';
import type { DeleteImagesResult } from '../types';

const LONG_PRESS_DURATION = 500;

type UseGallerySelectionOptions = {
  activeTab: string;
  onDeleteApplied: (pathsToAnimate: Set<string>) => void | Promise<void>;
};

type UseGallerySelectionResult = {
  isSelectionMode: boolean;
  selectedIds: Set<string>;
  showMenu: boolean;
  deletingIds: Set<string>;
  menuRef: RefObject<HTMLDivElement>;
  handleTouchStart: (imagePath: string, event: TouchEvent) => void;
  handleTouchMove: (event: TouchEvent) => void;
  handleTouchEnd: () => void;
  handleSelectionClick: (imagePath: string) => boolean;
  handleRefreshStart: () => void;
  handleDelete: () => Promise<void>;
  handleShare: () => Promise<void>;
  handleCancelSelection: () => void;
  toggleMenu: () => void;
};

export function useGallerySelection({ activeTab, onDeleteApplied }: UseGallerySelectionOptions): UseGallerySelectionResult {
  const [isSelectionMode, setIsSelectionMode] = useState(false);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [showMenu, setShowMenu] = useState(false);
  const [deletingIds, setDeletingIds] = useState<Set<string>>(new Set());
  const menuRef = useRef<HTMLDivElement>(null);
  const longPressTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const isSelectionModeRef = useRef(false);
  const touchStartPosRef = useRef<{ x: number; y: number } | null>(null);

  const clearTransientSelectionUiState = useCallback(() => {
    setShowMenu(false);
    setDeletingIds(new Set());
  }, []);

  const handleCancelSelection = useCallback(() => {
    setIsSelectionMode(false);
    setSelectedIds(new Set());
    clearTransientSelectionUiState();
  }, [clearTransientSelectionUiState]);

  const handleSelectionClick = useCallback((imagePath: string) => {
    if (!isSelectionMode) {
      return false;
    }

    setSelectedIds((prev) => {
      const next = new Set(prev);
      next.has(imagePath) ? next.delete(imagePath) : next.add(imagePath);
      if (next.size === 0) {
        setIsSelectionMode(false);
        clearTransientSelectionUiState();
      }
      return next;
    });
    return true;
  }, [clearTransientSelectionUiState, isSelectionMode]);

  const handleTouchStart = useCallback((imagePath: string, event: React.TouchEvent) => {
    const touch = event.touches[0];
    touchStartPosRef.current = { x: touch.clientX, y: touch.clientY };

    longPressTimerRef.current = setTimeout(() => {
      setIsSelectionMode(true);
      setSelectedIds(new Set([imagePath]));
    }, LONG_PRESS_DURATION);
  }, []);

  const handleTouchMove = useCallback((event: React.TouchEvent) => {
    if (!touchStartPosRef.current || !longPressTimerRef.current) {
      return;
    }

    const touch = event.touches[0];
    const dx = touch.clientX - touchStartPosRef.current.x;
    const dy = touch.clientY - touchStartPosRef.current.y;
    const distance = Math.sqrt(dx * dx + dy * dy);

    if (distance > 10) {
      clearTimeout(longPressTimerRef.current);
      longPressTimerRef.current = null;
      touchStartPosRef.current = null;
    }
  }, []);

  const handleTouchEnd = useCallback(() => {
    if (longPressTimerRef.current) {
      clearTimeout(longPressTimerRef.current);
      longPressTimerRef.current = null;
    }
    touchStartPosRef.current = null;
  }, []);

  const handleRefreshStart = useCallback(() => {
    handleCancelSelection();
  }, [handleCancelSelection]);

  const handleDelete = useCallback(async () => {
    if (selectedIds.size === 0) {
      return;
    }

    setShowMenu(false);

    const selectedAtDeleteStart = new Set(selectedIds);

    try {
      const resultJson = await window.GalleryAndroid?.deleteImages(JSON.stringify([...selectedAtDeleteStart]));
      if (!resultJson) {
        setShowMenu(false);
        return;
      }

      const result: DeleteImagesResult = JSON.parse(resultJson);
      const { deleted, notFound, failed } = result;
      const failureMessage = buildDeleteFailureMessage(result);

      const pathsToAnimate = new Set([...deleted, ...notFound]);
      const failedPaths = new Set(failed);

      if (pathsToAnimate.size === 0 && failedPaths.size > 0) {
        if (failureMessage) {
          toast.error(failureMessage);
        }
        return;
      }

      if (failedPaths.size > 0) {
        toast.error(`部分删除失败：${failedPaths.size} 张图片未删除。`);
      }

      setDeletingIds(pathsToAnimate);
      setShowMenu(false);

      await new Promise((resolve) => setTimeout(resolve, 300));

      await onDeleteApplied(pathsToAnimate);
      setDeletingIds(new Set());

      const remainingSelected = new Set([...selectedAtDeleteStart].filter((id) => !pathsToAnimate.has(id)));
      setSelectedIds(remainingSelected);
      if (remainingSelected.size === 0) {
        setIsSelectionMode(false);
      }
    } catch (err) {
      console.error('Delete failed:', err);
    }
  }, [onDeleteApplied, selectedIds]);

  const handleShare = useCallback(async () => {
    if (selectedIds.size === 0) {
      return;
    }

    try {
      await window.GalleryAndroid?.shareImages(JSON.stringify([...selectedIds]));
      setShowMenu(false);
    } catch (err) {
      console.error('Share failed:', err);
    }
  }, [selectedIds]);

  const toggleMenu = useCallback(() => {
    setShowMenu((prev) => !prev);
  }, []);

  useEffect(() => {
    return () => {
      if (longPressTimerRef.current) {
        clearTimeout(longPressTimerRef.current);
      }
    };
  }, []);

  useEffect(() => {
    isSelectionModeRef.current = isSelectionMode;
  }, [isSelectionMode]);

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(event.target as Node)) {
        setShowMenu(false);
      }
    };

    if (showMenu) {
      document.addEventListener('mousedown', handleClickOutside);
    }

    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
    };
  }, [showMenu]);

  useEffect(() => {
    if (isSelectionMode) {
      window.GalleryAndroid?.registerBackPressCallback?.();
    } else {
      window.GalleryAndroid?.unregisterBackPressCallback?.();
    }

    return () => {
      window.GalleryAndroid?.unregisterBackPressCallback?.();
    };
  }, [isSelectionMode]);

  useEffect(() => {
    const onBackPressed = () => {
      if (isSelectionModeRef.current) {
        handleCancelSelection();
      }
    };

    (window as Window & { __galleryOnBackPressed?: () => void }).__galleryOnBackPressed = onBackPressed;

    return () => {
      delete (window as Window & { __galleryOnBackPressed?: () => void }).__galleryOnBackPressed;
    };
  }, [handleCancelSelection]);

  useEffect(() => {
    if (activeTab !== 'gallery' && isSelectionMode) {
      handleCancelSelection();
    }
  }, [activeTab, isSelectionMode, handleCancelSelection]);

  return {
    isSelectionMode,
    selectedIds,
    showMenu,
    deletingIds,
    menuRef,
    handleTouchStart,
    handleTouchMove,
    handleTouchEnd,
    handleSelectionClick,
    handleRefreshStart,
    handleDelete,
    handleShare,
    handleCancelSelection,
    toggleMenu,
  };
}
