/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { memo, useCallback, useEffect, useState, useRef } from 'react';
import { RefreshCw, ImageOff, Loader2 } from 'lucide-react';
import { useConfigStore } from '../stores/configStore';
import type { GalleryImage } from '../types';

export const GalleryCard = memo(function GalleryCard() {
  const { config } = useConfigStore();
  const [images, setImages] = useState<GalleryImage[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [visibleImages, setVisibleImages] = useState<Set<number>>(new Set());
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
      // Sort by dateModified descending (newest first)
      parsed.sort((a, b) => b.dateModified - a.dateModified);
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

  const handleImageClick = useCallback((image: GalleryImage) => {
    if (window.PermissionAndroid?.openImageWithChooser) {
      window.PermissionAndroid.openImageWithChooser(image.path);
    }
  }, []);

  const handleRefresh = useCallback(() => {
    loadImages();
  }, [loadImages]);

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
          disabled={isLoading}
          className="flex items-center gap-1 px-3 py-1.5 text-sm text-gray-600 hover:text-gray-900 hover:bg-gray-100 rounded-lg transition-colors disabled:opacity-50"
        >
          <RefreshCw className={`w-4 h-4 ${isLoading ? 'animate-spin' : ''}`} />
          刷新
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
            className="aspect-square bg-gray-100 rounded-lg overflow-hidden cursor-pointer hover:opacity-90 transition-opacity"
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
          </div>
        ))}
      </div>
    </div>
  );
});
