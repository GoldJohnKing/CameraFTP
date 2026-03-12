/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { memo, useCallback, useEffect, useState, useRef } from 'react';
import { RefreshCw, ImageOff, Loader2, Check, X, Trash2, Share2, MoreVertical } from 'lucide-react';
import { toast } from 'sonner';
import { listen } from '@tauri-apps/api/event';
import { invoke, convertFileSrc } from '@tauri-apps/api/core';
import { useConfigStore } from '../stores/configStore';
import { permissionBridge } from '../types';
import type { GalleryImage, FileInfo, DeleteImagesResult } from '../types';

interface FileIndexChangedEvent {
  count: number;
  latestFilename: string | null;
}

export const GalleryCard = memo(function GalleryCard() {
  const { activeTab } = useConfigStore();
  const [images, setImages] = useState<GalleryImage[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [thumbnails, setThumbnails] = useState<Map<string, string>>(new Map());
  const [loadingThumbnails, setLoadingThumbnails] = useState<Set<string>>(new Set());
  const [isSelectionMode, setIsSelectionMode] = useState(false);
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [showMenu, setShowMenu] = useState(false);
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);
  const [deletingIds, setDeletingIds] = useState<Set<string>>(new Set());
  const menuRef = useRef<HTMLDivElement>(null);
  const observerRef = useRef<IntersectionObserver | null>(null);
  // Refs to track loading state without causing re-renders in observer callback
  const loadingThumbnailsRef = useRef<Set<string>>(new Set());
  const loadedThumbnailsRef = useRef<Set<string>>(new Set());

  // Load thumbnail for a specific image - defined before loadImages to avoid TDZ
  const loadThumbnail = useCallback(async (imagePath: string) => {
    // Skip if already loaded or loading (check refs for current state)
    if (loadedThumbnailsRef.current.has(imagePath) || loadingThumbnailsRef.current.has(imagePath)) {
      return;
    }

    // Mark as loading in ref (immediate, no re-render)
    loadingThumbnailsRef.current.add(imagePath);
    // Also update state for UI feedback
    setLoadingThumbnails(prev => new Set(prev).add(imagePath));

    try {
      const thumbnailPath = await window.GalleryAndroid?.getThumbnail(imagePath);
      if (thumbnailPath) {
        // Mark as loaded in ref
        loadedThumbnailsRef.current.add(imagePath);

        // Process thumbnail path: use Base64 directly if available, otherwise convert to asset:// URL
        let thumbnailUrl: string;
        if (thumbnailPath.startsWith('data:image/')) {
          // Fallback to Base64 format (for compatibility or cache failure)
          thumbnailUrl = thumbnailPath;
        } else {
          // Use Tauri's asset protocol to load local files
          thumbnailUrl = convertFileSrc(thumbnailPath);
        }

        // Update state for rendering
        setThumbnails(prev => new Map(prev).set(imagePath, thumbnailUrl));
      }
    } catch (err) {
      console.error('Failed to load thumbnail for imagePath:', imagePath, err);
    } finally {
      // Remove from loading set
      loadingThumbnailsRef.current.delete(imagePath);
      setLoadingThumbnails(prev => {
        const next = new Set(prev);
        next.delete(imagePath);
        return next;
      });
    }
  }, []); // No dependencies - uses refs for state checks

  const loadImages = useCallback(async () => {
    if (!window.GalleryAndroid) {
      return;
    }

    setIsLoading(true);
    setError(null);
    // Don't clear thumbnails here - let the useEffect handle it when images change
    // This prevents race conditions between clear and load

    try {
      // Use Rust command to scan gallery images
      const files = await invoke<FileInfo[]>('scan_gallery_images');
      // Convert FileInfo to GalleryImage format
      const galleryImages: GalleryImage[] = files.map(file => ({
        path: file.path,
        filename: file.filename,
        sortTime: Number(file.sortTime), // bigint to number
      }));
      setImages(galleryImages);
      // Thumbnail loading is handled by the useEffect that watches images array
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load images');
      setImages([]);
    } finally {
      setIsLoading(false);
    }
  }, []);

  // Load images on mount
  useEffect(() => {
    loadImages();
  }, [loadImages]);

  // Force thumbnail reload when images array changes (after refresh)
  useEffect(() => {
    if (images.length > 0 && !isLoading) {
      // Reset thumbnail loading state for all images to ensure fresh load
      loadingThumbnailsRef.current.clear();
      loadedThumbnailsRef.current.clear();
      
      // Clear any stale thumbnail data that doesn't match current images
      const currentPaths = new Set(images.map(img => img.path));
      setThumbnails(prev => {
        const next = new Map();
        prev.forEach((value, key) => {
          if (currentPaths.has(key)) {
            next.set(key, value);
          }
        });
        return next;
      });
      
      // Preload first 9 images after a short delay to ensure DOM is ready
      requestAnimationFrame(() => {
        const imagesToPreload = images.slice(0, 9);
        imagesToPreload.forEach((image, index) => {
          setTimeout(() => {
            // Force reload by clearing the loaded state first
            loadedThumbnailsRef.current.delete(image.path);
            loadingThumbnailsRef.current.delete(image.path);
            loadThumbnail(image.path);
          }, index * 50);
        });
      });
    }
  }, [images, isLoading, loadThumbnail]);

  // Listen for file index changes
  useEffect(() => {
    const unlistenPromise = listen<FileIndexChangedEvent>('file-index-changed', () => {
      // Refresh the gallery when files change
      loadImages();
    });

    return () => {
      unlistenPromise.then((unlisten) => unlisten()).catch(() => {});
    };
  }, [loadImages]);

  // Setup intersection observer for lazy loading thumbnails
  // Observer is created once and uses loadThumbnail which tracks state via refs
  useEffect(() => {
    // Batch process visible images to reduce re-renders
    const pendingLoads = new Set<string>();
    let loadTimeout: ReturnType<typeof setTimeout> | null = null;

    const processPendingLoads = () => {
      loadTimeout = null;
      pendingLoads.forEach((path) => {
        if (!loadedThumbnailsRef.current.has(path) && !loadingThumbnailsRef.current.has(path)) {
          loadThumbnail(path);
        }
      });
      pendingLoads.clear();
    };

    observerRef.current = new IntersectionObserver(
      (entries) => {
        entries.forEach((entry) => {
          const path = entry.target.getAttribute('data-path');
          if (entry.isIntersecting && path) {
            pendingLoads.add(path);
          }
        });

        // Batch delayed loading to reduce main thread pressure
        if (pendingLoads.size > 0 && !loadTimeout) {
          loadTimeout = setTimeout(processPendingLoads, 50);
        }
      },
      { 
        rootMargin: '200px', // Increase preload range to 200px
        threshold: 0.01 // Trigger as soon as it enters rootMargin
      }
    );

    return () => {
      if (loadTimeout) {
        clearTimeout(loadTimeout);
      }
      observerRef.current?.disconnect();
    };
  }, [loadThumbnail]); // loadThumbnail is stable (no deps), so this runs once

  // Observe image elements - only observe, don't load immediately (avoid duplicate with preload)
  const imageRefCallback = useCallback((el: HTMLDivElement | null) => {
    if (el && observerRef.current) {
      observerRef.current.observe(el);
      // Note: don't load here, let IntersectionObserver handle visible images
      // Preload logic is in loadImages via requestAnimationFrame with delay
    }
  }, []);

  const longPressTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const LONG_PRESS_DURATION = 500;

  const handleTouchStart = useCallback((image: GalleryImage, e: React.TouchEvent) => {
    // Prevent default long-press behavior (drag preview, context menu)
    e.preventDefault();
    
    longPressTimerRef.current = setTimeout(() => {
      setIsSelectionMode(true);
      setSelectedIds(new Set([image.path]));
    }, LONG_PRESS_DURATION);
  }, []);

  const handleTouchEnd = useCallback(() => {
    if (longPressTimerRef.current) {
      clearTimeout(longPressTimerRef.current);
      longPressTimerRef.current = null;
    }
  }, []);

  // Cleanup long-press timer on unmount to prevent memory leak
  useEffect(() => {
    return () => {
      if (longPressTimerRef.current) {
        clearTimeout(longPressTimerRef.current);
      }
    };
  }, []);

  const handleImageClick = useCallback((image: GalleryImage) => {
    if (isSelectionMode) {
      setSelectedIds(prev => {
        const next = new Set(prev);
        next.has(image.path) ? next.delete(image.path) : next.add(image.path);
        if (next.size === 0) {
          setIsSelectionMode(false);
        }
        return next;
      });
    } else if (window.PermissionAndroid?.openImageWithChooser) {
      window.PermissionAndroid.openImageWithChooser(image.path);
    }
  }, [isSelectionMode]);

  const handleRefresh = useCallback(async () => {
    // Check storage permission first on Android
    if (permissionBridge.isAvailable()) {
      const permissions = await permissionBridge.checkAll();
      if (permissions && !permissions.storage) {
        permissionBridge.requestStorage();
        return;
      }
    }

    // Cancel selection mode when refreshing
    setIsSelectionMode(false);
    setSelectedIds(new Set());
    setShowMenu(false);

    setIsRefreshing(true);
    const startTime = Date.now();

    try {
      await loadImages();
    } finally {
      // Ensure animation lasts at least 200ms so users can see the refresh effect
      const elapsed = Date.now() - startTime;
      const remaining = Math.max(0, 200 - elapsed);

      setTimeout(() => {
        setIsRefreshing(false);
      }, remaining);
    }
  }, [loadImages]);

  const handleDelete = useCallback(() => {
    if (selectedIds.size === 0) return;
    setShowDeleteConfirm(true);
    setShowMenu(false);
  }, [selectedIds.size]);

  const handleDeleteConfirm = useCallback(async (confirmed: boolean) => {
    if (!confirmed) {
      setShowDeleteConfirm(false);
      return;
    }

    try {
      const resultJson = await window.GalleryAndroid?.deleteImages(JSON.stringify([...selectedIds]));
      if (!resultJson) {
        toast.error('删除失败');
        setShowDeleteConfirm(false);
        return;
      }

      const result: DeleteImagesResult = JSON.parse(resultJson);
      const { deleted, notFound, failed } = result;

      const pathsToAnimate = new Set([...deleted, ...notFound]);
      const failedPaths = new Set(failed);

      if (pathsToAnimate.size === 0 && failedPaths.size > 0) {
        toast.error('删除失败');
        setShowDeleteConfirm(false);
        return;
      }

      setDeletingIds(pathsToAnimate);
      setShowDeleteConfirm(false);
      setShowMenu(false);

      await new Promise(resolve => setTimeout(resolve, 300));

      if (pathsToAnimate.size > 0) {
        await window.GalleryAndroid?.removeThumbnails(JSON.stringify([...pathsToAnimate]));
      }

      setImages(prev => prev.filter(img => !pathsToAnimate.has(img.path)));
      setThumbnails(prev => {
        const next = new Map(prev);
        pathsToAnimate.forEach(path => next.delete(path));
        return next;
      });
      setDeletingIds(new Set());

      const remainingSelected = new Set([...selectedIds].filter(id => !pathsToAnimate.has(id)));
      setSelectedIds(remainingSelected);
      if (remainingSelected.size === 0) {
        setIsSelectionMode(false);
      }

      if (failedPaths.size > 0) {
        toast.success(`已删除 ${pathsToAnimate.size} 张图片，${failedPaths.size} 张删除失败`);
      } else {
        toast.success(`已删除 ${pathsToAnimate.size} 张图片`);
      }
    } catch (err) {
      console.error('Delete failed:', err);
      toast.error('删除失败');
      setShowDeleteConfirm(false);
    }
  }, [selectedIds]);

  const handleShare = useCallback(async () => {
    if (selectedIds.size === 0) return;
    
    try {
      await window.GalleryAndroid?.shareImages(JSON.stringify([...selectedIds]));
      setShowMenu(false);
    } catch (err) {
      console.error('Share failed:', err);
      toast.error('分享失败');
    }
  }, [selectedIds]);

  const handleCancelSelection = useCallback(() => {
    setIsSelectionMode(false);
    setSelectedIds(new Set());
    setShowMenu(false);
  }, []);

  // Ref to track selection mode for back press callback
  // This ref ensures the onBackPressed callback always sees the latest state
  // even when the callback is called from Android without triggering a re-render
  const isSelectionModeRef = useRef(false);
  
  // Sync ref with state on every render
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

  // Register/unregister back press callback when selection mode changes
  useEffect(() => {
    // Register back press callback when entering selection mode
    if (isSelectionMode) {
      window.GalleryAndroid?.registerBackPressCallback?.();
    } else {
      window.GalleryAndroid?.unregisterBackPressCallback?.();
    }

    // Cleanup on unmount
    return () => {
      window.GalleryAndroid?.unregisterBackPressCallback?.();
    };
  }, [isSelectionMode]);

  // Expose onBackPressed callback for Android to call
  useEffect(() => {
    // Define the callback function - use ref to always get latest state
    const onBackPressed = () => {
      if (isSelectionModeRef.current) {
        handleCancelSelection();
      }
    };

    // Attach to window as global function (WebView JS interface doesn't allow adding properties to the bridge object)
    (window as Window & { __galleryOnBackPressed?: () => void }).__galleryOnBackPressed = onBackPressed;

    return () => {
      // Cleanup: remove the callback when component unmounts
      delete (window as Window & { __galleryOnBackPressed?: () => void }).__galleryOnBackPressed;
    };
  }, [handleCancelSelection]);

  // Cancel selection when switching to other tabs
  useEffect(() => {
    if (activeTab !== 'gallery' && isSelectionMode) {
      handleCancelSelection();
    }
  }, [activeTab, isSelectionMode, handleCancelSelection]);

  // Not on Android
  if (!window.GalleryAndroid) {
    return null;
  }

  // Empty state (remains visible during refresh, only button changes)
  if (images.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center py-20">
        <ImageOff className="w-12 h-12 text-gray-300" />
        <p className="mt-3 text-gray-500">暂无图片</p>
        <button
          onClick={handleRefresh}
          disabled={isLoading || isRefreshing}
          className="mt-4 flex items-center gap-2 px-4 py-2 text-blue-600 hover:bg-blue-50 rounded-lg transition-colors disabled:opacity-50"
        >
          <RefreshCw className={`w-4 h-4 ${isLoading || isRefreshing ? 'animate-spin' : ''}`} />
          <span>{isLoading || isRefreshing ? '刷新中...' : '刷新'}</span>
        </button>
      </div>
    );
  }

  // Error state
  if (error) {
    return (
      <div className="flex flex-col items-center justify-center py-20">
        <p className="text-red-500">{error}</p>
        <button
          onClick={handleRefresh}
          disabled={isRefreshing}
          className="mt-4 flex items-center gap-2 px-4 py-2 text-blue-600 hover:bg-blue-50 rounded-lg transition-colors disabled:opacity-50"
        >
          <RefreshCw className={`w-4 h-4 ${isRefreshing ? 'animate-spin' : ''}`} />
          <span>{isRefreshing ? '刷新中...' : '重试'}</span>
        </button>
      </div>
    );
  }

  return (
    <div className="space-y-3 pt-6 select-none">
      {/* Header with refresh button */}
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold text-gray-900">
          图库 ({images.length})
        </h2>
        <button
          onClick={handleRefresh}
          disabled={isRefreshing}
          className="text-sm text-blue-500 hover:text-blue-600 flex items-center gap-1.5 disabled:opacity-50 transition-colors"
        >
          <RefreshCw className={`w-4 h-4 ${isRefreshing ? 'animate-spin' : ''}`} />
          <span>{isRefreshing ? '刷新中...' : '刷新'}</span>
        </button>
      </div>

      {/* Image grid */}
      <div className="grid grid-cols-3 gap-1.5">
        {images.map((image) => {
          const thumbnail = thumbnails.get(image.path);
          const isLoadingThumb = loadingThumbnails.has(image.path);
          const isDeleting = deletingIds.has(image.path);

          return (
            <div
              key={image.path}
              data-path={image.path}
              ref={imageRefCallback}
              onClick={() => handleImageClick(image)}
              onTouchStart={(e) => handleTouchStart(image, e)}
              onTouchEnd={handleTouchEnd}
              onTouchMove={handleTouchEnd}
              onTouchCancel={handleTouchEnd}
              onContextMenu={(e) => e.preventDefault()}
              className={`aspect-square bg-gray-100 rounded-lg overflow-hidden cursor-pointer hover:opacity-90 transition-all duration-300 relative select-none ${
                isSelectionMode && selectedIds.has(image.path) ? 'ring-2 ring-blue-500' : ''
              } ${isDeleting ? 'scale-0 opacity-0' : 'scale-100 opacity-100'}`}
            >
              {thumbnail ? (
                <img
                  src={thumbnail}
                  alt={image.filename}
                  className="w-full h-full object-cover pointer-events-none"
                  loading="lazy"
                  draggable={false}
                />
              ) : isLoadingThumb ? (
                <div className="w-full h-full flex items-center justify-center bg-gray-200">
                  <Loader2 className="w-6 h-6 text-gray-400 animate-spin" />
                </div>
              ) : (
                <div className="w-full h-full flex items-center justify-center">
                  <div className="w-8 h-8 bg-gray-200 rounded animate-pulse" />
                </div>
              )}

              {isSelectionMode && (
                <div className={`absolute top-2 left-2 w-6 h-6 rounded-full flex items-center justify-center ${
                  selectedIds.has(image.path)
                    ? 'bg-blue-500'
                    : 'bg-black/30 border-2 border-white/70'
                }`}>
                  {selectedIds.has(image.path) && (
                    <Check className="w-4 h-4 text-white" />
                  )}
                </div>
              )}
            </div>
          );
        })}
      </div>

      {/* FAB and Menu for selection mode */}
      {isSelectionMode && (
        <div className="fixed bottom-20 right-4 z-50" ref={menuRef}>
          {/* Menu */}
          {showMenu && (
            <div className="absolute bottom-16 right-0 bg-white rounded-xl shadow-xl min-w-[140px] overflow-hidden mb-2 select-none">
              <button
                onClick={handleDelete}
                disabled={selectedIds.size === 0}
                className="w-full flex items-center gap-3 px-4 py-3 text-left hover:bg-gray-50 disabled:opacity-50 disabled:cursor-not-allowed"
              >
                <Trash2 className="w-5 h-5 text-red-500" />
                <span>删除({selectedIds.size})</span>
              </button>
              <button
                onClick={handleShare}
                disabled={selectedIds.size === 0}
                className="w-full flex items-center gap-3 px-4 py-3 text-left hover:bg-gray-50 disabled:opacity-50 disabled:cursor-not-allowed border-t border-gray-100"
              >
                <Share2 className="w-5 h-5 text-blue-500" />
                <span>分享({selectedIds.size})</span>
              </button>
              <button
                onClick={handleCancelSelection}
                className="w-full flex items-center gap-3 px-4 py-3 text-left hover:bg-gray-50 border-t border-gray-100"
              >
                <X className="w-5 h-5 text-gray-500" />
                <span>取消选择</span>
              </button>
            </div>
          )}
          
          {/* FAB */}
          <button
            onClick={() => setShowMenu(prev => !prev)}
            className="w-14 h-14 rounded-full bg-blue-500 shadow-lg flex items-center justify-center text-white hover:bg-blue-600 transition-colors"
          >
            <MoreVertical className="w-6 h-6" />
          </button>
          
          {/* Badge */}
          {selectedIds.size > 0 && (
            <div className="absolute -top-1 -right-1 w-6 h-6 rounded-full bg-red-500 text-white text-xs flex items-center justify-center font-medium">
              {selectedIds.size > 99 ? '99+' : selectedIds.size}
            </div>
          )}
        </div>
      )}

      {/* Delete confirmation dialog */}
      {showDeleteConfirm && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-[100]">
          <div className="bg-white rounded-xl p-6 max-w-sm mx-4 shadow-xl">
            <h3 className="text-lg font-semibold text-gray-900 mb-2">
              确认删除
            </h3>
            <p className="text-gray-600 mb-6">
              确定要删除选中的 {selectedIds.size} 张图片吗？此操作无法撤销。
            </p>
            <div className="flex gap-3 justify-end">
              <button
                onClick={() => handleDeleteConfirm(false)}
                className="px-4 py-2 text-gray-700 bg-gray-100 rounded-lg hover:bg-gray-200 transition-colors"
              >
                取消
              </button>
              <button
                onClick={() => handleDeleteConfirm(true)}
                className="px-4 py-2 text-white bg-red-500 rounded-lg hover:bg-red-600 transition-colors"
              >
                删除
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
});
