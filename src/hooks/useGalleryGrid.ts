/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { convertFileSrc } from '@tauri-apps/api/core';
import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import type { GalleryImage } from '../types';

const GRID_MOVE_DURATION_MS = 220;
const GRID_ENTER_DURATION_MS = 200;
const GRID_MOVE_EASING = 'cubic-bezier(0.22, 1, 0.36, 1)';

type UseGalleryGridOptions = {
  images: GalleryImage[];
  isLoading: boolean;
  enteringIds: Set<string>;
  suppressGridAnimations: boolean;
};

type UseGalleryGridResult = {
  thumbnails: Map<string, string>;
  loadingThumbnails: Set<string>;
  imageRefCallback: (imagePath: string, el: HTMLDivElement | null) => void;
  removeThumbnailEntries: (imagePaths: Set<string>) => void;
  cleanupDeletedThumbnails: (imagePaths: Set<string>) => Promise<void>;
};

export function useGalleryGrid({
  images,
  isLoading,
  enteringIds,
  suppressGridAnimations,
}: UseGalleryGridOptions): UseGalleryGridResult {
  const [thumbnails, setThumbnails] = useState<Map<string, string>>(new Map());
  const [loadingThumbnails, setLoadingThumbnails] = useState<Set<string>>(new Set());
  const observerRef = useRef<IntersectionObserver | null>(null);
  const tileRefs = useRef(new Map<string, HTMLDivElement>());
  const previousPositionsRef = useRef(new Map<string, DOMRect>());
  const loadingThumbnailsRef = useRef<Set<string>>(new Set());
  const loadedThumbnailsRef = useRef<Set<string>>(new Set());
  const preloadTimeoutsRef = useRef<Array<ReturnType<typeof setTimeout>>>([]);
  const preloadAnimationFrameRef = useRef<number | null>(null);

  const animateGridMovement = useCallback((element: HTMLDivElement, oldRect: DOMRect, newRect: DOMRect) => {
    const deltaX = oldRect.left - newRect.left;
    const deltaY = oldRect.top - newRect.top;

    if (deltaX === 0 && deltaY === 0) {
      return;
    }

    element.style.transition = 'none';
    element.style.transform = `translate(${deltaX}px, ${deltaY}px)`;

    requestAnimationFrame(() => {
      element.style.transition = `transform ${GRID_MOVE_DURATION_MS}ms ${GRID_MOVE_EASING}`;
      element.style.transform = '';
    });
  }, []);

  const animateGridEntry = useCallback((element: HTMLDivElement) => {
    element.style.transition = 'none';
    element.style.transform = 'scale(0.88)';
    element.style.opacity = '0';

    requestAnimationFrame(() => {
      element.style.transition = [
        `transform ${GRID_ENTER_DURATION_MS}ms ${GRID_MOVE_EASING}`,
        `opacity ${GRID_ENTER_DURATION_MS}ms ease-out`,
      ].join(', ');
      element.style.transform = '';
      element.style.opacity = '1';
    });
  }, []);

  const loadThumbnail = useCallback(async (imagePath: string) => {
    if (loadedThumbnailsRef.current.has(imagePath) || loadingThumbnailsRef.current.has(imagePath)) {
      return;
    }

    loadingThumbnailsRef.current.add(imagePath);
    setLoadingThumbnails(prev => new Set(prev).add(imagePath));

    try {
      const thumbnailPath = await window.GalleryAndroid?.getThumbnail(imagePath);
      if (!thumbnailPath) {
        return;
      }

      loadedThumbnailsRef.current.add(imagePath);
      const thumbnailUrl = thumbnailPath.startsWith('data:image/') ? thumbnailPath : convertFileSrc(thumbnailPath);
      setThumbnails(prev => new Map(prev).set(imagePath, thumbnailUrl));
    } catch (err) {
      console.error('Failed to load thumbnail for imagePath:', imagePath, err);
    } finally {
      loadingThumbnailsRef.current.delete(imagePath);
      setLoadingThumbnails(prev => {
        const next = new Set(prev);
        next.delete(imagePath);
        return next;
      });
    }
  }, []);

  useEffect(() => {
    let isCancelled = false;
    const currentPaths = new Set(images.map(image => image.path));

    preloadTimeoutsRef.current.forEach((timeoutId) => {
      clearTimeout(timeoutId);
    });
    preloadTimeoutsRef.current = [];
    if (preloadAnimationFrameRef.current !== null) {
      cancelAnimationFrame(preloadAnimationFrameRef.current);
      preloadAnimationFrameRef.current = null;
    }

    loadingThumbnailsRef.current.forEach((path) => {
      if (!currentPaths.has(path)) {
        loadingThumbnailsRef.current.delete(path);
      }
    });

    loadedThumbnailsRef.current = new Set(
      [...loadedThumbnailsRef.current].filter(path => currentPaths.has(path)),
    );

    setLoadingThumbnails(prev => new Set([...prev].filter(path => currentPaths.has(path))));
    setThumbnails((prev) => {
      const next = new Map<string, string>();
      prev.forEach((value, key) => {
        if (currentPaths.has(key)) {
          next.set(key, value);
        }
      });
      return next;
    });

    if (images.length > 0 && !isLoading) {
      preloadAnimationFrameRef.current = requestAnimationFrame(() => {
        preloadAnimationFrameRef.current = null;
        images.slice(0, 9).forEach((image, index) => {
          const timeoutId = setTimeout(() => {
            void loadThumbnail(image.path);
          }, index * 50);
          preloadTimeoutsRef.current.push(timeoutId);
        });
      });
    }

    const galleryBridge = window.GalleryAndroid;
    if (galleryBridge && !isLoading) {
      void (async () => {
        try {
          await galleryBridge.cleanupThumbnailsNotInList(JSON.stringify([...currentPaths]));
        } catch (cleanupErr) {
          if (!isCancelled) {
            console.debug('Thumbnail cleanup skipped or failed:', cleanupErr);
          }
        }
      })();
    }

    return () => {
      isCancelled = true;
      preloadTimeoutsRef.current.forEach((timeoutId) => {
        clearTimeout(timeoutId);
      });
      preloadTimeoutsRef.current = [];
      if (preloadAnimationFrameRef.current !== null) {
        cancelAnimationFrame(preloadAnimationFrameRef.current);
        preloadAnimationFrameRef.current = null;
      }
    };
  }, [images, isLoading, loadThumbnail]);

  useLayoutEffect(() => {
    const currentPositions = new Map<string, DOMRect>();

    images.forEach((image) => {
      const element = tileRefs.current.get(image.path);
      if (!element) {
        return;
      }

      const newRect = element.getBoundingClientRect();
      currentPositions.set(image.path, newRect);

      if (suppressGridAnimations) {
        return;
      }

      const previousRect = previousPositionsRef.current.get(image.path);
      if (previousRect) {
        animateGridMovement(element, previousRect, newRect);
      } else if (enteringIds.has(image.path)) {
        animateGridEntry(element);
      }
    });

    previousPositionsRef.current = currentPositions;
  }, [images, enteringIds, suppressGridAnimations, animateGridEntry, animateGridMovement]);

  useEffect(() => {
    const pendingLoads = new Set<string>();
    let loadTimeout: ReturnType<typeof setTimeout> | null = null;

    const processPendingLoads = () => {
      loadTimeout = null;
      pendingLoads.forEach((path) => {
        if (!loadedThumbnailsRef.current.has(path) && !loadingThumbnailsRef.current.has(path)) {
          void loadThumbnail(path);
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

        if (pendingLoads.size > 0 && !loadTimeout) {
          loadTimeout = setTimeout(processPendingLoads, 50);
        }
      },
      {
        rootMargin: '200px',
        threshold: 0.01,
      },
    );

    return () => {
      if (loadTimeout) {
        clearTimeout(loadTimeout);
      }
      observerRef.current?.disconnect();
    };
  }, [loadThumbnail]);

  const imageRefCallback = useCallback((imagePath: string, el: HTMLDivElement | null) => {
    const previousElement = tileRefs.current.get(imagePath);
    if (previousElement && observerRef.current) {
      observerRef.current.unobserve(previousElement);
    }

    if (!el) {
      tileRefs.current.delete(imagePath);
      return;
    }

    tileRefs.current.set(imagePath, el);
    observerRef.current?.observe(el);
  }, []);

  const removeThumbnailEntries = useCallback((imagePaths: Set<string>) => {
    if (imagePaths.size === 0) {
      return;
    }

    setThumbnails((prev) => {
      const next = new Map(prev);
      imagePaths.forEach((path) => next.delete(path));
      return next;
    });

    setLoadingThumbnails((prev) => {
      const next = new Set(prev);
      imagePaths.forEach((path) => next.delete(path));
      return next;
    });

    imagePaths.forEach((path) => {
      loadingThumbnailsRef.current.delete(path);
      loadedThumbnailsRef.current.delete(path);
    });
  }, []);

  const cleanupDeletedThumbnails = useCallback(async (imagePaths: Set<string>) => {
    removeThumbnailEntries(imagePaths);
    if (imagePaths.size === 0) {
      return;
    }

    try {
      await window.GalleryAndroid?.removeThumbnails(JSON.stringify([...imagePaths]));
    } catch (err) {
      console.debug('Failed to remove deleted thumbnails:', err);
    }
  }, [removeThumbnailEntries]);

  return {
    thumbnails,
    loadingThumbnails,
    imageRefCallback,
    removeThumbnailEntries,
    cleanupDeletedThumbnails,
  };
}
