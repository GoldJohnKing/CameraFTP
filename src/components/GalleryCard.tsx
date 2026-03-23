/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { memo, useCallback, useEffect } from 'react';
import { RefreshCw, ImageOff, X, Trash2, Share2, MoreVertical } from 'lucide-react';
import { useConfigStore } from '../stores/configStore';
import type { MediaItemDto } from '../types/gallery-v2';
import { isGalleryMediaAvailable } from '../services/gallery-media';
import { useGalleryPager } from '../hooks/useGalleryPager';
import { useThumbnailScheduler } from '../hooks/useThumbnailScheduler';
import { useGallerySelection } from '../hooks/useGallerySelection';
import { useImagePreviewOpener } from '../hooks/useImagePreviewOpener';
import { VirtualGalleryGrid } from './VirtualGalleryGrid';

export const GalleryCard = memo(function GalleryCard() {
  const { activeTab } = useConfigStore();
  const openPreview = useImagePreviewOpener();
  const pager = useGalleryPager();
  const scheduler = useThumbnailScheduler();

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
    onDeleteApplied: async (idsToDelete) => {
      pager.removeItems(idsToDelete);
      scheduler.removeThumbs(idsToDelete);
    },
  });

  // Load first page on mount
  useEffect(() => {
    void pager.loadNextPage();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Register media metadata with scheduler when items change
  useEffect(() => {
    if (pager.items.length > 0) {
      scheduler.registerMedia(pager.items);
    }
  }, [pager.items, scheduler]);

  const handleRangeChange = useCallback(
    (visibleIds: string[], nearbyIds: string[]) => {
      scheduler.updateViewport(visibleIds, nearbyIds);
    },
    [scheduler],
  );

  const handleItemClick = useCallback(
    (item: MediaItemDto) => {
      if (handleSelectionClick(item.mediaId)) {
        return;
      }

      void openPreview({
        filePath: item.uri,
        allUris: pager.items.map((i) => i.uri),
      });
    },
    [handleSelectionClick, openPreview, pager.items],
  );

  const handleRefresh = useCallback(async () => {
    handleRefreshStart();
    scheduler.cleanup();
    await pager.reload();
  }, [handleRefreshStart, pager, scheduler]);

  // Not on Android
  if (!isGalleryMediaAvailable()) {
    return null;
  }

  // Error state
  if (pager.error) {
    return (
      <div className="flex flex-col items-center justify-center py-20">
        <p className="text-red-500">{pager.error}</p>
        <button
          onClick={handleRefresh}
          disabled={pager.isLoading}
          className="mt-4 flex items-center gap-2 px-4 py-2 text-blue-600 hover:bg-blue-50 rounded-lg transition-colors disabled:opacity-50"
        >
          <RefreshCw className={`w-4 h-4 ${pager.isLoading ? 'animate-spin' : ''}`} />
          <span>{pager.isLoading ? '刷新中...' : '重试'}</span>
        </button>
      </div>
    );
  }

  // Empty state
  if (pager.items.length === 0 && !pager.isLoading) {
    return (
      <div className="flex flex-col items-center justify-center py-20">
        <ImageOff className="w-12 h-12 text-gray-300" />
        <p className="mt-3 text-gray-500">暂无图片</p>
        <button
          onClick={handleRefresh}
          disabled={pager.isLoading}
          className="mt-4 flex items-center gap-2 px-4 py-2 text-blue-600 hover:bg-blue-50 rounded-lg transition-colors disabled:opacity-50"
        >
          <RefreshCw className={`w-4 h-4 ${pager.isLoading ? 'animate-spin' : ''}`} />
          <span>{pager.isLoading ? '刷新中...' : '刷新'}</span>
        </button>
      </div>
    );
  }

  return (
    <div className="space-y-3 pt-6 select-none">
      {/* Header with refresh button */}
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold text-gray-900">
          图库 ({pager.items.length})
        </h2>
        <button
          onClick={handleRefresh}
          disabled={pager.isLoading}
          className="text-sm text-blue-500 hover:text-blue-600 flex items-center gap-1.5 disabled:opacity-50 transition-colors"
        >
          <RefreshCw className={`w-4 h-4 ${pager.isLoading ? 'animate-spin' : ''}`} />
          <span>{pager.isLoading ? '刷新中...' : '刷新'}</span>
        </button>
      </div>

      {/* Virtualized image grid */}
      <VirtualGalleryGrid
        items={pager.items}
        thumbnails={scheduler.thumbnails}
        loadingThumbs={scheduler.loadingThumbs}
        onItemClick={handleItemClick}
        onRangeChange={handleRangeChange}
        isSelectionMode={isSelectionMode}
        selectedIds={selectedIds}
        deletingIds={deletingIds}
        onTouchStart={handleTouchStart}
        onTouchEnd={handleTouchEnd}
      />

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
