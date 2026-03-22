/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { memo, useCallback } from 'react';
import { RefreshCw, ImageOff, Loader2, Check, X, Trash2, Share2, MoreVertical } from 'lucide-react';
import { useConfigStore } from '../stores/configStore';
import type { GalleryImage } from '../types';
import { isGalleryMediaAvailable } from '../services/gallery-media';
import { useGalleryLibrary } from '../hooks/useGalleryLibrary';
import { useGalleryGrid } from '../hooks/useGalleryGrid';
import { useGallerySelection } from '../hooks/useGallerySelection';
import { useImagePreviewOpener } from '../hooks/useImagePreviewOpener';

const GRID_EXIT_DURATION_MS = 180;

export const GalleryCard = memo(function GalleryCard() {
  const { activeTab } = useConfigStore();
  const openPreview = useImagePreviewOpener();
  const {
    images,
    isLoading,
    isRefreshing,
    error,
    enteringIds,
    suppressGridAnimations,
    refresh,
    removeImages,
  } = useGalleryLibrary();
  const { thumbnails, loadingThumbnails, imageRefCallback, cleanupDeletedThumbnails } = useGalleryGrid({
    images,
    isLoading,
    enteringIds,
    suppressGridAnimations,
  });

  const {
    isSelectionMode,
    selectedIds,
    showMenu,
    deletingIds,
    menuRef,
    handleTouchStart,
    handleTouchEnd,
    handleSelectionClick,
    handleRefreshStart,
    handleDelete,
    handleShare,
    handleCancelSelection,
    toggleMenu,
  } = useGallerySelection({
    activeTab,
    onDeleteApplied: async (pathsToAnimate) => {
      removeImages(pathsToAnimate);
      await cleanupDeletedThumbnails(pathsToAnimate);
    },
  });

  const handleImageClick = useCallback((image: GalleryImage) => {
    if (handleSelectionClick(image.path)) {
      return;
    }

    void openPreview({
      filePath: image.path,
      allUris: images.map((img) => img.path),
    });
  }, [handleSelectionClick, images, openPreview]);

  const handleRefresh = useCallback(async () => {
    // Cancel selection mode when refreshing
    await refresh({
      onStart: () => {
        handleRefreshStart();
      },
    });
  }, [handleRefreshStart, refresh]);

  // Not on Android
  if (!isGalleryMediaAvailable()) {
    return null;
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
              ref={(el) => imageRefCallback(image.path, el)}
              onClick={() => handleImageClick(image)}
              onTouchStart={(e) => handleTouchStart(image.path, e)}
              onTouchEnd={handleTouchEnd}
              onTouchMove={handleTouchEnd}
              onTouchCancel={handleTouchEnd}
              onContextMenu={(e) => e.preventDefault()}
              className={`aspect-square bg-gray-100 rounded-lg overflow-hidden cursor-pointer hover:opacity-90 transition-opacity duration-200 relative select-none ${
                isSelectionMode && selectedIds.has(image.path) ? 'ring-2 ring-blue-500' : ''
              } ${isDeleting ? 'scale-[0.88] opacity-0' : 'scale-100 opacity-100'}`}
              style={{
                transitionDuration: isDeleting ? `${GRID_EXIT_DURATION_MS}ms` : undefined,
              }}
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
                onClick={() => void handleDelete()}
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
            onClick={toggleMenu}
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
