/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { memo, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { ImageOff, X, Trash2, Share2, Sparkles, MoreVertical, Palette, ChevronDown } from 'lucide-react';
import { useConfigStore } from '../stores/configStore';
import { usePermissionStore } from '../stores/permissionStore';
import type { MediaItemDto, GalleryItemsAddedEvent, GalleryItemsDeletedEvent } from '../types';
import { isGalleryV2Available, invalidateMediaIds } from '../services/gallery-media-v2';
import { GALLERY_REFRESH_REQUESTED_EVENT } from '../utils/gallery-refresh';
import { withMinDuration } from '../utils/format';
import { permissionBridge } from '../services/permission-bridge';
import { useGalleryPager } from '../hooks/useGalleryPager';
import { useThumbnailScheduler } from '../hooks/useThumbnailScheduler';
import { useGallerySelection } from '../hooks/useGallerySelection';
import { useImagePreviewOpener } from '../hooks/useImagePreviewOpener';
import { useAndroidAutoOpenLatestPhoto } from '../hooks/useAndroidAutoOpenLatestPhoto';
import { VirtualGalleryGrid, type VirtualGalleryGridHandle } from './VirtualGalleryGrid';
import { RefreshButton } from './ui';
import { PromptDialog } from './PromptDialog';
import { ColorGradingDialog } from './ColorGradingDialog';
import { enqueueColorGrading } from '../hooks/useColorGradingProgress';
import { useColorGradingPresets } from '../hooks/useColorGradingPresets';
import { isRawFile } from '../utils/raw';
import { getAiEditCallContext } from '../utils/ai-edit';
import { classifyFile, FILTER_CATEGORY_ORDER, FILTER_CATEGORY_LABEL, type GalleryFilterMode, type GalleryFileCategory } from '../utils/gallery-filter';
import { DateJumpDialog, type GalleryDateOption } from './DateJumpDialog';
import { formatDateTitle, toDateKey } from '../utils/date-format';

export const GalleryCard = memo(function GalleryCard() {
  const activeTab = useConfigStore((state) => state.activeTab);
  const draft = useConfigStore((state) => state.draft);
  const openPreview = useImagePreviewOpener();
  const pager = useGalleryPager();
  const scheduler = useThumbnailScheduler();

  useAndroidAutoOpenLatestPhoto({
    galleryItems: pager.items,
    openMethod: draft?.androidImageViewer?.openMethod,
    autoOpenLatestWhenVisible: draft?.androidImageViewer?.autoOpenLatestWhenVisible,
  });

  const getUriForId = useCallback(
    (mediaId: string) => pager.items.find((item) => item.mediaId === mediaId)?.uri,
    [pager.items]
  );

  const {
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
    isDragSelectingRef,
    dragAnchorIndexRef,
    handleDragSelect,
  } = useGallerySelection({
    activeTab,
    onDeleteApplied: async (idsToDelete) => {
      pager.removeItems(idsToDelete);
      scheduler.removeThumbs(idsToDelete);
      // Invalidate disk cache for deleted media IDs
      await invalidateMediaIds([...idsToDelete]);
    },
    getUriForId,
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
  // ===== Extension filter =====
  const [filterMode, setFilterMode] = useState<GalleryFilterMode>('all');
  // Categories that actually have at least one loaded item; drives which
  // filter buttons are shown (empty categories stay hidden).
  const availableCategories = useMemo<Set<GalleryFileCategory>>(() => {
    const set = new Set<GalleryFileCategory>();
    for (const item of pager.items) set.add(classifyFile(item));
    return set;
  }, [pager.items]);
  // Items after applying the active extension filter. When "all" (default) the
  // full set is returned by reference so existing behavior is unchanged.
  const filteredItems = useMemo<MediaItemDto[]>(() => {
    if (filterMode === 'all') return pager.items;
    return pager.items.filter((item) => classifyFile(item) === filterMode);
  }, [pager.items, filterMode]);
  // If the active category empties out (e.g. its items were deleted), fall back
  // to "all" so the gallery never shows a blank grid with a hidden filter.
  useEffect(() => {
    if (filterMode !== 'all' && !availableCategories.has(filterMode)) {
      setFilterMode('all');
    }
  }, [filterMode, availableCategories]);

  // First visible item's capture day — drives the title and the date-jump
  // default selection. Only the calendar day is tracked, so scrolling within
  // the same day never triggers a re-render.
  const [firstVisible, setFirstVisible] = useState<{ key: string; ms: number } | null>(null);
  // mediaId → dateModifiedMs lookup, rebuilt when items change, for O(1) date
  // resolution of the topmost visible item reported by the grid.
  const dateByMediaId = useMemo(() => {
    const map = new Map<string, number>();
    for (const item of filteredItems) map.set(item.mediaId, item.dateModifiedMs);
    return map;
  }, [filteredItems]);

  const handleRangeChange = useCallback(
    (visibleIds: string[], nearbyIds: string[]) => {
      scheduler.updateViewport(visibleIds, nearbyIds);
      const firstId = visibleIds[0];
      if (firstId) {
        const ms = dateByMediaId.get(firstId);
        if (ms != null) {
          const key = toDateKey(ms);
          setFirstVisible((prev) => (prev?.key === key ? prev : { key, ms }));
        }
      }
    },
    [scheduler, dateByMediaId],
  );

  const handleNearEnd = useCallback(() => {
    // Load next page when scrolling near the end
    if (!pager.isLoading && pager.cursor !== null) {
      void pager.loadNextPage();
    }
  }, [pager]);

  const handleItemClick = useCallback(
    (item: MediaItemDto) => {
      if (handleSelectionClick(item.mediaId)) {
        return;
      }

      void openPreview({
        filePath: item.uri,
        allUris: filteredItems.map((i) => i.uri),
      }).catch(() => {
        // Silently ignore preview open failures (same as LatestPhotoCard)
      });
    },
    [handleSelectionClick, openPreview, filteredItems],
  );

  const requestStoragePermission = usePermissionStore((state) => state.requestStoragePermission);
  const startPermissionPolling = usePermissionStore((state) => state.startPolling);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [showColorGradingDialog, setShowColorGradingDialog] = useState(false);
  const colorGradingPresets = useColorGradingPresets();

  const handleRefresh = useCallback(async () => {
    // Check permissions before loading — request if not granted
    if (permissionBridge.isAvailable()) {
      const permissions = await permissionBridge.checkAll();
      if (permissions && !permissions.storage) {
        requestStoragePermission();
        startPermissionPolling('storage');
        return;
      }
    }

    setIsRefreshing(true);

    try {
      await withMinDuration(async () => {
        handleRefreshStart();
        // Reset the extension filter so a refresh always starts from "全部".
        setFilterMode('all');
        scheduler.cleanup();
        await pager.reload();
      });
    } finally {
      setIsRefreshing(false);
    }
  }, [handleRefreshStart, pager, scheduler, requestStoragePermission, startPermissionPolling]);

  const handleColorGrading = useCallback(() => {
    toggleMenu();
    setShowColorGradingDialog(true);
  }, [toggleMenu]);

  const handleColorGradingConfirm = useCallback(async (lutId: string, meteringMode: string, evOffset: number) => {
    setShowColorGradingDialog(false);
    const filePaths = Array.from(selectedIds)
      .map(id => pager.items.find(item => item.mediaId === id))
      .filter((item): item is NonNullable<typeof item> => item != null)
      .map(item => window.ImageViewerAndroid?.resolveFilePath?.(item.uri) ?? item.uri);
    if (filePaths.length > 0) {
      await enqueueColorGrading(filePaths, lutId, meteringMode, evOffset);
    }
    handleCancelSelection();
  }, [selectedIds, pager.items, handleCancelSelection]);

  // mediaId → item lookup, rebuilt when items change, so per-selection RAW
  // checks stay O(selected) instead of scanning the full list per render.
  const itemByMediaId = useMemo(() => {
    const map = new Map<string, MediaItemDto>();
    for (const item of pager.items) map.set(item.mediaId, item);
    return map;
  }, [pager.items]);

  const hasRawSelected = useMemo(
    () => Array.from(selectedIds).some((id) => {
      const item = itemByMediaId.get(id);
      return item?.filePath ? isRawFile(item.filePath) : false;
    }),
    [selectedIds, itemByMediaId],
  );

  // ===== Date-jump picker =====
  const gridRef = useRef<VirtualGalleryGridHandle>(null);
  const [showDateJump, setShowDateJump] = useState(false);
  const [isLoadingDates, setIsLoadingDates] = useState(false);
  // mediaId of the cell to pulse after a date-jump; the grid owns the pulse
  // lifecycle (arming it once the cell mounts after the scroll settles).
  const [highlightMediaId, setHighlightMediaId] = useState<string | null>(null);

  // Unique capture days, newest-first (filteredItems is sorted dateDesc), with the
  // photo count per day. When a filter is active, only matching items are included.
  const dateOptions = useMemo<GalleryDateOption[]>(() => {
    const order: string[] = [];
    const msByKey = new Map<string, number>();
    const countByKey = new Map<string, number>();
    for (const item of filteredItems) {
      const key = toDateKey(item.dateModifiedMs);
      if (!msByKey.has(key)) {
        msByKey.set(key, item.dateModifiedMs);
        order.push(key);
        countByKey.set(key, 0);
      }
      countByKey.set(key, (countByKey.get(key) ?? 0) + 1);
    }
    return order.map((key) => ({ key, ms: msByKey.get(key)!, count: countByKey.get(key) ?? 0 }));
  }, [filteredItems]);

  // Title reflects the first visible photo's day; fall back to the newest photo
  // before the first range report lands (e.g. initial mount / remeasure).
  const titleMs = firstVisible?.ms ?? filteredItems[0]?.dateModifiedMs ?? null;
  const selectedDateKey = firstVisible?.key ?? (filteredItems[0] ? toDateKey(filteredItems[0].dateModifiedMs) : null);

  const handleOpenDateJump = useCallback(async () => {
    setShowDateJump(true);
    // Load every remaining page so the date list covers all loaded items.
    if (pager.cursor !== null) {
      setIsLoadingDates(true);
      try {
        await pager.loadAll();
      } finally {
        setIsLoadingDates(false);
      }
    }
  }, [pager.cursor, pager.loadAll]);

  const handleDateJump = useCallback((key: string) => {
    setShowDateJump(false);
    const index = filteredItems.findIndex((item) => toDateKey(item.dateModifiedMs) === key);
    if (index >= 0) {
      const targetId = filteredItems[index].mediaId;
      gridRef.current?.scrollToIndex(index);
      // Request a highlight pulse on the landed-on cell. The grid owns the
      // pulse lifecycle (it waits for the cell to mount after the scroll
      // settles, then plays the animation and clears itself), so we only set
      // the request here. Clearing any prior request first ensures a fresh
      // pulse even when jumping to the same day twice in a row.
      setHighlightMediaId(null);
      // Defer to the next tick so React registers a changed value (null→id)
      // even when the previous highlight was the same id.
      requestAnimationFrame(() => setHighlightMediaId(targetId));
    }
  }, [filteredItems]);

  // Full gallery refresh on any gallery-refresh-requested event (upload, delete, permission grant, etc.)
  useEffect(() => {
    const handleGalleryRefresh = () => {
      void handleRefresh();
    };
    window.addEventListener(GALLERY_REFRESH_REQUESTED_EVENT, handleGalleryRefresh);
    return () => {
      window.removeEventListener(GALLERY_REFRESH_REQUESTED_EVENT, handleGalleryRefresh);
    };
  }, [handleRefresh]);

  // Listen for incremental delete events from ImageViewerActivity (preserves scroll position)
  useEffect(() => {
    const handleItemsDeleted = (event: GalleryItemsDeletedEvent) => {
      const { mediaIds } = event.detail;
      if (mediaIds?.length > 0) {
        const idsToDelete = new Set(mediaIds);
        pager.removeItems(idsToDelete);
        scheduler.removeThumbs(idsToDelete);
        // Invalidate disk cache for deleted media IDs
        void invalidateMediaIds([...idsToDelete]);
      }
    };

    window.addEventListener('gallery-items-deleted', handleItemsDeleted as EventListener);
    return () => {
      window.removeEventListener('gallery-items-deleted', handleItemsDeleted as EventListener);
    };
  }, [pager, scheduler]);

  // Listen for incremental add events from FTP upload (preserves scroll position)
  useEffect(() => {
    const handleItemsAdded = (event: GalleryItemsAddedEvent) => {
      const { items } = event.detail;
      if (items?.length > 0) {
        // Add new items to the beginning of the list (newest first)
        pager.addItems(items);
      }
    };

    window.addEventListener('gallery-items-added', handleItemsAdded as EventListener);
    return () => {
      window.removeEventListener('gallery-items-added', handleItemsAdded as EventListener);
    };
  }, [pager]);

  // Not on Android
  if (!isGalleryV2Available()) {
    return null;
  }

  // Error state
  if (pager.error) {
    return (
      <div className="flex flex-col items-center justify-center py-20">
        <p className="text-red-500">{pager.error}</p>
        <RefreshButton
          onClick={handleRefresh}
          isLoading={isRefreshing}
          label="重试"
          className="mt-4 flex items-center gap-2 px-4 py-2 text-blue-600 hover:bg-blue-50 rounded-lg transition-colors disabled:opacity-50"
        />
      </div>
    );
  }

  // Empty state
  if (pager.items.length === 0 && !pager.isLoading) {
    return (
      <div className="flex flex-col items-center justify-center py-20">
        <ImageOff className="w-12 h-12 text-gray-300" />
        <p className="mt-3 text-gray-500">暂无图片</p>
        <RefreshButton
          onClick={handleRefresh}
          isLoading={isRefreshing}
          label="刷新"
          className="mt-4 flex items-center gap-2 px-4 py-2 text-blue-600 hover:bg-blue-50 rounded-lg transition-colors disabled:opacity-50"
        />
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col px-4 pt-6 pb-[68px] select-none">
      {/* Header with refresh button */}
      <div className="flex items-center justify-between shrink-0">
        <button
          type="button"
          onClick={() => void handleOpenDateJump()}
          disabled={titleMs == null}
          data-testid="gallery-date-title"
          className="flex min-w-0 items-center gap-1 text-lg font-semibold text-gray-900 transition-colors hover:text-blue-600 active:text-blue-700 disabled:opacity-50"
        >
          <span className="truncate">
            {titleMs != null ? formatDateTitle(titleMs) : '图库'}
          </span>
          <ChevronDown className="h-4 w-4 shrink-0 text-gray-400" />
        </button>
        <RefreshButton onClick={handleRefresh} isLoading={isRefreshing} />
      </div>
      {/* Extension filter — only categories with loaded files get a button */}
      <div className="flex items-center gap-1.5 shrink-0 overflow-x-auto py-1.5 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
        {(['all', ...FILTER_CATEGORY_ORDER.filter((c) => availableCategories.has(c))] as GalleryFilterMode[]).map((mode) => {
          const label = mode === 'all' ? '全部' : FILTER_CATEGORY_LABEL[mode];
          const active = filterMode === mode;
          return (
            <button
              key={mode}
              type="button"
              onClick={() => setFilterMode(mode)}
              data-testid={`gallery-filter-${mode}`}
              className={`shrink-0 px-3 py-1 rounded-full text-xs font-medium transition-colors ${active ? 'bg-blue-500 text-white' : 'bg-white text-gray-600 border border-gray-200'}`}
            >
              {label}
            </button>
          );
        })}
      </div>

      {/* Virtualized image grid */}
      <div className="flex-1 min-h-0 mt-2">
        <VirtualGalleryGrid
          ref={gridRef}
          items={filteredItems}
          thumbnails={scheduler.thumbnails}
          loadingThumbs={scheduler.loadingThumbs}
          onItemClick={handleItemClick}
          onRangeChange={handleRangeChange}
          onNearEnd={handleNearEnd}
          isSelectionMode={isSelectionMode}
          selectedIds={selectedIds}
          deletingIds={deletingIds}
          onTouchStart={handleTouchStart}
          onTouchMove={handleTouchMove}
          onTouchEnd={handleTouchEnd}
          onDragSelect={handleDragSelect}
          isDragSelectingRef={isDragSelectingRef}
          dragAnchorIndexRef={dragAnchorIndexRef}
          highlightMediaId={highlightMediaId}
        />
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
                onClick={handleAiEdit}
                disabled={selectedIds.size === 0}
                className="w-full flex items-center gap-3 px-4 py-3 text-left hover:bg-gray-50 disabled:opacity-50 disabled:cursor-not-allowed border-t border-gray-100"
              >
                  <Sparkles className="w-5 h-5 text-amber-600" />
                  <span>修图({selectedIds.size})</span>
                </button>
              <button
                onClick={handleColorGrading}
                disabled={selectedIds.size === 0 || !hasRawSelected}
                className="w-full flex items-center gap-3 px-4 py-3 text-left hover:bg-gray-50 disabled:opacity-50 disabled:cursor-not-allowed border-t border-gray-100"
              >
                <Palette className="w-5 h-5 text-violet-600" />
                <span>调色({selectedIds.size})</span>
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

      {/* AI修图提示词对话框 */}
      <PromptDialog
        isOpen={showAiEditPrompt}
        defaultPrompt={draft?.aiEdit?.manualPrompt || ''}
        defaultModel={draft?.aiEdit?.manualModel || undefined}
        autoEditEnabled={getAiEditCallContext(draft).autoEdit}
        hasApiKey={getAiEditCallContext(draft).hasApiKey}
        onConfirm={handleAiEditPromptConfirm}
        onCancel={handleCancelAiEditPrompt}
      />
      <ColorGradingDialog
        isOpen={showColorGradingDialog}
        colorGradingPresets={colorGradingPresets}
        onConfirm={handleColorGradingConfirm}
        onCancel={() => setShowColorGradingDialog(false)}
      />
      <DateJumpDialog
        isOpen={showDateJump}
        dates={dateOptions}
        selectedKey={selectedDateKey}
        isLoading={isLoadingDates}
        onSelect={handleDateJump}
        onClose={() => setShowDateJump(false)}
      />
    </div>
  );
});
