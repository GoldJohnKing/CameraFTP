/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useCallback, useEffect, useRef, useState, type RefObject } from 'react';
import { toast } from 'sonner';
import { buildDeleteFailureMessage } from '../utils/gallery-delete';
import { useConfigStore } from '../stores/configStore';
import { enqueueAiEdit } from './useAiEditProgress';
import type { DeleteImagesResult } from '../types';

const LONG_PRESS_DURATION = 400; // Android ViewConfiguration.DEFAULT_LONG_PRESS_TIMEOUT
const TOUCH_MOVE_THRESHOLD = 15; // Movement threshold to cancel long press (px)

type UseGallerySelectionOptions = {
  activeTab: string;
  onDeleteApplied: (pathsToAnimate: Set<string>) => void | Promise<void>;
  getUriForId: (mediaId: string) => string | undefined;
};

type UseGallerySelectionResult = {
  isSelectionMode: boolean;
  selectedIds: Set<string>;
  showMenu: boolean;
  deletingIds: Set<string>;
  showAiEditPrompt: boolean;
  menuRef: RefObject<HTMLDivElement>;
  handleTouchStart: (imagePath: string, event: React.TouchEvent, isScrolling: boolean) => void;
  handleTouchMove: (event: React.TouchEvent) => void;
  handleTouchEnd: () => void;
  handleSelectionClick: (imagePath: string) => boolean;
  handleRefreshStart: () => void;
  handleDelete: () => Promise<void>;
  handleShare: () => Promise<void>;
  handleAiEdit: () => void;
  handleAiEditPromptConfirm: (prompt: string, model: string, saveAsAutoEdit: boolean) => Promise<void>;
  handleCancelAiEditPrompt: () => void;
  handleCancelSelection: () => void;
  toggleMenu: () => void;
};

export function useGallerySelection({ activeTab, onDeleteApplied, getUriForId }: UseGallerySelectionOptions): UseGallerySelectionResult {
  const [isSelectionMode, setIsSelectionMode] = useState(false);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [showMenu, setShowMenu] = useState(false);
  const [deletingIds, setDeletingIds] = useState<Set<string>>(new Set());
  const [showAiEditPrompt, setShowAiEditPrompt] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);
  const longPressTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const isSelectionModeRef = useRef(false);
  const touchStartPosRef = useRef<{ x: number; y: number } | null>(null);
  const wasScrollingAtTouchStartRef = useRef(false);

  const clearTransientSelectionUiState = useCallback(() => {
    setShowMenu(false);
    setDeletingIds(new Set());
  }, []);

  const handleCancelSelection = useCallback(() => {
    setIsSelectionMode(false);
    isSelectionModeRef.current = false;
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
        isSelectionModeRef.current = false;
        clearTransientSelectionUiState();
      }
      return next;
    });
    return true;
  }, [clearTransientSelectionUiState, isSelectionMode]);

  const handleTouchStart = useCallback((imagePath: string, event: React.TouchEvent, isScrolling: boolean) => {
    // Ignore multi-finger touches (e.g., three-finger screenshot gesture)
    if (event.touches.length > 1) {
      return;
    }

    // If the list is scrolling, this touch is for stopping the scroll only.
    // Never trigger long press in this case, regardless of how long the user holds.
    if (isScrolling) {
      wasScrollingAtTouchStartRef.current = true;
      return;
    }

    // Reset the flag - this is a fresh touch on a stationary list
    wasScrollingAtTouchStartRef.current = false;

    const touch = event.touches[0];
    touchStartPosRef.current = { x: touch.clientX, y: touch.clientY };

    longPressTimerRef.current = setTimeout(() => {
      // Double-check that this touch didn't start during scrolling
      if (!wasScrollingAtTouchStartRef.current) {
        setIsSelectionMode(true);
        isSelectionModeRef.current = true;
        setSelectedIds(new Set([imagePath]));
      }
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

    if (distance > TOUCH_MOVE_THRESHOLD) {
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
    wasScrollingAtTouchStartRef.current = false;
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

    // Convert mediaIds to URIs for the delete call
    const urisToDelete = [...selectedAtDeleteStart]
      .map((mediaId) => getUriForId(mediaId))
      .filter((uri): uri is string => uri !== undefined);

    if (urisToDelete.length === 0) {
      return;
    }

    try {
      const resultJson = await window.GalleryAndroid?.deleteImages(JSON.stringify(urisToDelete));
      if (!resultJson) {
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

      // Map URIs back to mediaIds for animation and removal
      const mediaIdsToAnimate = new Set<string>();
      const uriToMediaId = new Map([...selectedAtDeleteStart].map((id) => [getUriForId(id), id]));
      for (const uri of pathsToAnimate) {
        const mediaId = uriToMediaId.get(uri);
        if (mediaId) {
          mediaIdsToAnimate.add(mediaId);
        }
      }

      setDeletingIds(mediaIdsToAnimate);

      await new Promise((resolve) => setTimeout(resolve, 300));

      await onDeleteApplied(mediaIdsToAnimate);
      setDeletingIds(new Set());

      const remainingSelected = new Set([...selectedAtDeleteStart].filter((id) => !mediaIdsToAnimate.has(id)));
      setSelectedIds(remainingSelected);
      if (remainingSelected.size === 0) {
        setIsSelectionMode(false);
        isSelectionModeRef.current = false;
      }
    } catch (err) {
      console.error('Delete failed:', err);
    }
  }, [onDeleteApplied, selectedIds, getUriForId]);

  const handleShare = useCallback(async () => {
    if (selectedIds.size === 0) {
      return;
    }

    try {
      // Convert mediaIds to URIs (same pattern as handleDelete)
      const urisToShare = [...selectedIds]
        .map((mediaId) => getUriForId(mediaId))
        .filter((uri): uri is string => uri !== undefined);

      if (urisToShare.length === 0) {
        return;
      }

      await window.GalleryAndroid?.shareImages(JSON.stringify(urisToShare));
      setShowMenu(false);
    } catch (err) {
      console.error('Share failed:', err);
    }
  }, [selectedIds, getUriForId]);

  const handleAiEdit = useCallback(() => {
    if (selectedIds.size === 0) {
      return;
    }
    setShowMenu(false);
    setShowAiEditPrompt(true);
  }, [selectedIds]);

  const handleAiEditPromptConfirm = useCallback(async (prompt: string, model: string, saveAsAutoEdit: boolean) => {
    setShowAiEditPrompt(false);

    const draft = useConfigStore.getState().draft;
    if (draft) {
      useConfigStore.getState().updateDraft(d => ({
        ...d,
        aiEdit: {
          ...d.aiEdit,
          manualPrompt: prompt,
          manualModel: model,
          ...(saveAsAutoEdit ? {
            prompt,
            provider: {
              ...d.aiEdit.provider,
              model,
            },
          } : {}),
        },
      }));
    }

    const uris = [...selectedIds]
      .map((mediaId) => getUriForId(mediaId))
      .filter((uri): uri is string => uri !== undefined);

    if (uris.length === 0) {
      return;
    }

    const filePaths = uris
      .map((uri) => window.ImageViewerAndroid?.resolveFilePath?.(uri) ?? uri);

    await enqueueAiEdit(filePaths, prompt, model);
  }, [selectedIds, getUriForId]);

  const handleCancelAiEditPrompt = useCallback(() => {
    setShowAiEditPrompt(false);
  }, []);

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

    window.__galleryOnBackPressed = onBackPressed;

    return () => {
      delete window.__galleryOnBackPressed;
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
    showAiEditPrompt,
    menuRef,
    handleTouchStart,
    handleTouchMove,
    handleTouchEnd,
    handleSelectionClick,
    handleRefreshStart,
    handleDelete,
    handleShare,
    handleAiEdit,
    handleAiEditPromptConfirm,
    handleCancelAiEditPrompt,
    handleCancelSelection,
    toggleMenu,
  };
}
