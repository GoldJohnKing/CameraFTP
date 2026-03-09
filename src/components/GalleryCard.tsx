/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { memo, useCallback, useEffect, useState, useRef } from 'react';
import { RefreshCw, ImageOff, Loader2, Check, X, Trash2, Share2, MoreVertical } from 'lucide-react';
import { listen } from '@tauri-apps/api/event';
import { useConfigStore } from '../stores/configStore';
import type { GalleryImage } from '../types';

interface FileIndexChangedEvent {
  count: number;
  latestFilename: string | null;
}

export const GalleryCard = memo(function GalleryCard() {
  const { config } = useConfigStore();
  const [images, setImages] = useState<GalleryImage[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [visibleImages, setVisibleImages] = useState<Set<number>>(new Set());
  const [isSelectionMode, setIsSelectionMode] = useState(false);
  const [selectedIds, setSelectedIds] = useState<Set<number>>(new Set());
  const [showMenu, setShowMenu] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);
  const observerRef = useRef<IntersectionObserver | null>(null);

  const loadImages = useCallback(async () => {
    if (!config?.savePath || !window.GalleryAndroid) {
      return;
    }

    setIsLoading(true);
    setError(null);

    try {
      const result = await window.GalleryAndroid.getGalleryImages(config.savePath);
      const response = JSON.parse(result) as { images: GalleryImage[] };
      const parsed = response.images;
      // Sort by EXIF-based sortTime descending (newest first)
      parsed.sort((a, b) => b.sortTime - a.sortTime);
      setImages(parsed);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load images');
      setImages([]);
    } finally {
      setIsLoading(false);
    }
  }, [config?.savePath]);

  // Load images on mount
  useEffect(() => {
    loadImages();
  }, [loadImages]);

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

  // Setup intersection observer for lazy loading
  useEffect(() => {
    observerRef.current = new IntersectionObserver(
      (entries) => {
        entries.forEach((entry) => {
          const id = Number(entry.target.getAttribute('data-id'));
          if (entry.isIntersecting) {
            setVisibleImages((prev) => new Set(prev).add(id));
          }
        });
      },
      { rootMargin: '100px' }
    );

    return () => {
      observerRef.current?.disconnect();
    };
  }, []);

  // Observe image elements
  const imageRefCallback = useCallback((el: HTMLDivElement | null) => {
    if (el && observerRef.current) {
      observerRef.current.observe(el);
    }
  }, []);

  const longPressTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const LONG_PRESS_DURATION = 500;

  const handleTouchStart = useCallback((image: GalleryImage) => {
    longPressTimerRef.current = setTimeout(() => {
      setIsSelectionMode(true);
      setSelectedIds(new Set([image.id]));
    }, LONG_PRESS_DURATION);
  }, []);

  const handleTouchEnd = useCallback(() => {
    if (longPressTimerRef.current) {
      clearTimeout(longPressTimerRef.current);
      longPressTimerRef.current = null;
    }
  }, []);

  const handleImageClick = useCallback((image: GalleryImage) => {
    if (isSelectionMode) {
      setSelectedIds(prev => {
        const next = new Set(prev);
        next.has(image.id) ? next.delete(image.id) : next.add(image.id);
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
    setIsRefreshing(true);
    const startTime = Date.now();

    try {
      await loadImages();
    } finally {
      // 确保动画至少持续 200ms，让用户能看到刷新效果
      const elapsed = Date.now() - startTime;
      const remaining = Math.max(0, 200 - elapsed);

      setTimeout(() => {
        setIsRefreshing(false);
      }, remaining);
    }
  }, [loadImages]);

  const handleDelete = useCallback(async () => {
    if (selectedIds.size === 0) return;
    
    if (confirm(`确定删除 ${selectedIds.size} 张图片？`)) {
      const success = await window.GalleryAndroid?.deleteImages(JSON.stringify([...selectedIds]));
      if (success) {
        loadImages();
        setIsSelectionMode(false);
        setSelectedIds(new Set());
        setShowMenu(false);
      }
    }
  }, [selectedIds, loadImages]);

  const handleShare = useCallback(async () => {
    if (selectedIds.size === 0) return;
    
    await window.GalleryAndroid?.shareImages(JSON.stringify([...selectedIds]));
    setShowMenu(false);
  }, [selectedIds]);

  const handleCancelSelection = useCallback(() => {
    setIsSelectionMode(false);
    setSelectedIds(new Set());
    setShowMenu(false);
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

  // Not on Android
  if (!window.GalleryAndroid) {
    return null;
  }

  // Loading state
  if (isLoading && images.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center py-20">
        <Loader2 className="w-8 h-8 text-blue-600 animate-spin" />
        <p className="mt-3 text-gray-500">加载中...</p>
      </div>
    );
  }

  // Empty state
  if (!isLoading && images.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center py-20">
        <ImageOff className="w-12 h-12 text-gray-300" />
        <p className="mt-3 text-gray-500">暂无图片</p>
        <button
          onClick={handleRefresh}
          className="mt-4 flex items-center gap-2 px-4 py-2 text-blue-600 hover:bg-blue-50 rounded-lg transition-colors"
        >
          <RefreshCw className="w-4 h-4" />
          刷新
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
          className="mt-4 flex items-center gap-2 px-4 py-2 text-blue-600 hover:bg-blue-50 rounded-lg transition-colors"
        >
          <RefreshCw className="w-4 h-4" />
          重试
        </button>
      </div>
    );
  }

  return (
    <div className="space-y-3 pt-6">
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
      <div className="grid grid-cols-3 gap-1">
        {images.map((image) => (
          <div
            key={image.id}
            data-id={image.id}
            ref={imageRefCallback}
            onClick={() => handleImageClick(image)}
            onTouchStart={() => handleTouchStart(image)}
            onTouchEnd={handleTouchEnd}
            onTouchCancel={handleTouchEnd}
            className={`aspect-square bg-gray-100 rounded-lg overflow-hidden cursor-pointer hover:opacity-90 transition-opacity relative ${
              isSelectionMode && selectedIds.has(image.id) ? 'ring-2 ring-blue-500' : ''
            }`}
          >
            {visibleImages.has(image.id) ? (
              <img
                src={image.thumbnail}
                alt={image.filename}
                className="w-full h-full object-cover"
                loading="lazy"
              />
            ) : (
              <div className="w-full h-full flex items-center justify-center">
                <div className="w-8 h-8 bg-gray-200 rounded animate-pulse" />
              </div>
            )}
            
            {isSelectionMode && (
              <div className={`absolute top-2 left-2 w-6 h-6 rounded-full flex items-center justify-center ${
                selectedIds.has(image.id)
                  ? 'bg-blue-500'
                  : 'bg-black/30 border-2 border-white/70'
              }`}>
                {selectedIds.has(image.id) && (
                  <Check className="w-4 h-4 text-white" />
                )}
              </div>
            )}
          </div>
        ))}
      </div>

      {/* FAB and Menu for selection mode */}
      {isSelectionMode && (
        <div className="fixed bottom-20 right-4 z-50" ref={menuRef}>
          {/* Menu */}
          {showMenu && (
            <div className="absolute bottom-16 right-0 bg-white rounded-xl shadow-xl min-w-[140px] overflow-hidden mb-2">
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
    </div>
  );
});
