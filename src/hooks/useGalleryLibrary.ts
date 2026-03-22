/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useCallback, useEffect, useRef, useState } from 'react';
import type { GalleryImage } from '../types';
import { permissionBridge } from '../types';
import { usePermissionStore } from '../stores/permissionStore';
import {
  GALLERY_REFRESH_REQUESTED_EVENT,
  requestLatestPhotoRefresh,
} from '../utils/gallery-refresh';
import { listGalleryMedia } from '../services/gallery-media';

const MIN_REFRESH_SPINNER_MS = 200;
const GRID_ENTER_DURATION_MS = 200;

type RefreshOptions = {
  onStart?: () => void;
};

type LoadImagesOptions = {
  showLoading?: boolean;
  suppressGridAnimations?: boolean;
};

export function useGalleryLibrary() {
  const requestStoragePermission = usePermissionStore((state) => state.requestStoragePermission);
  const startPermissionPolling = usePermissionStore((state) => state.startPolling);
  const [images, setImages] = useState<GalleryImage[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [isRefreshing, setIsRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [enteringIds, setEnteringIds] = useState<Set<string>>(new Set());
  const [suppressGridAnimations, setSuppressGridAnimations] = useState(false);
  const previousImagePathsRef = useRef<Set<string>>(new Set());
  const enteringTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const loadImages = useCallback(async (options?: LoadImagesOptions) => {
    if (!window.GalleryAndroid) {
      return;
    }

    const showLoading = options?.showLoading ?? true;
    const shouldSuppressGridAnimations = options?.suppressGridAnimations ?? false;

    if (showLoading) {
      setIsLoading(true);
    }
    setSuppressGridAnimations(shouldSuppressGridAnimations);
    setError(null);

    try {
      const galleryImages = await listGalleryMedia();
      const previousPaths = previousImagePathsRef.current;
      const nextPaths = new Set(galleryImages.map((image) => image.path));
      const newPaths = previousPaths.size === 0
        ? []
        : galleryImages.filter((image) => !previousPaths.has(image.path)).map((image) => image.path);

      if (enteringTimeoutRef.current) {
        clearTimeout(enteringTimeoutRef.current);
        enteringTimeoutRef.current = null;
      }

      setEnteringIds(new Set(newPaths));
      if (newPaths.length > 0) {
        enteringTimeoutRef.current = setTimeout(() => {
          setEnteringIds(new Set());
          enteringTimeoutRef.current = null;
        }, GRID_ENTER_DURATION_MS + 80);
      }

      setImages(galleryImages);
      previousImagePathsRef.current = nextPaths;

    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load images');
      setImages([]);
    } finally {
      if (showLoading) {
        setIsLoading(false);
      }
    }
  }, []);

  useEffect(() => {
    void loadImages({ showLoading: true, suppressGridAnimations: false });

    const handler = () => {
      void loadImages({ showLoading: false, suppressGridAnimations: true });
    };

    window.addEventListener(GALLERY_REFRESH_REQUESTED_EVENT, handler);
    return () => {
      window.removeEventListener(GALLERY_REFRESH_REQUESTED_EVENT, handler);
      if (enteringTimeoutRef.current) {
        clearTimeout(enteringTimeoutRef.current);
      }
    };
  }, [loadImages]);

  useEffect(() => {
    const reloadWhenVisible = () => {
      if (document.visibilityState !== 'visible') {
        return;
      }

      void loadImages({ showLoading: false, suppressGridAnimations: true });
    };

    window.addEventListener('focus', reloadWhenVisible);
    document.addEventListener('visibilitychange', reloadWhenVisible);

    return () => {
      window.removeEventListener('focus', reloadWhenVisible);
      document.removeEventListener('visibilitychange', reloadWhenVisible);
    };
  }, [loadImages]);

  const refresh = useCallback(async (options?: RefreshOptions) => {
    if (permissionBridge.isAvailable()) {
      const permissions = await permissionBridge.checkAll();
      if (permissions && !permissions.storage) {
        requestStoragePermission();
        startPermissionPolling('storage');
        return;
      }
    }

    options?.onStart?.();
    setIsRefreshing(true);
    const startTime = Date.now();

    try {
      await loadImages({ showLoading: true, suppressGridAnimations: false });
      requestLatestPhotoRefresh({ reason: 'manual' });
    } finally {
      const elapsed = Date.now() - startTime;
      const remaining = Math.max(0, MIN_REFRESH_SPINNER_MS - elapsed);
      setTimeout(() => {
        setIsRefreshing(false);
      }, remaining);
    }
  }, [loadImages, requestStoragePermission, startPermissionPolling]);

  const removeImages = useCallback((pathsToRemove: Set<string>) => {
    if (pathsToRemove.size === 0) {
      return;
    }

    setImages((prev) => prev.filter((image) => !pathsToRemove.has(image.path)));
    previousImagePathsRef.current = new Set(
      [...previousImagePathsRef.current].filter((path) => !pathsToRemove.has(path)),
    );
    setEnteringIds((prev) => new Set([...prev].filter((path) => !pathsToRemove.has(path))));
  }, []);

  return {
    images,
    isLoading,
    isRefreshing,
    error,
    enteringIds,
    suppressGridAnimations,
    refresh,
    removeImages,
  };
}
