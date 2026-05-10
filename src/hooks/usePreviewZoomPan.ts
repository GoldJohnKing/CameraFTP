/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';

export function usePreviewZoomPan(imagePath: string | null) {
  const transformRef = useRef({ scale: 1, panX: 0, panY: 0 });
  const imgRef = useRef<HTMLImageElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const isDraggingRef = useRef(false);
  const dragStartRef = useRef({ x: 0, y: 0 });
  const wheelEndTimerRef = useRef<ReturnType<typeof setTimeout>>(undefined);
  const appWindow = useMemo(() => getCurrentWindow(), []);

  // React state only for toolbar display — updated when interaction settles
  const [displayScale, setDisplayScale] = useState(1);
  const [isDragging, setIsDragging] = useState(false);

  const applyTransform = useCallback(() => {
    const img = imgRef.current;
    const container = containerRef.current;
    if (img) {
      const { scale, panX, panY } = transformRef.current;
      img.style.transform = `translate(${panX}px, ${panY}px) scale(${scale})`;
    }
    if (container) {
      const { scale } = transformRef.current;
      container.style.cursor = isDraggingRef.current
        ? 'grabbing'
        : scale > 1
          ? 'grab'
          : 'default';
    }
  }, []);

  const syncDisplayScale = useCallback(() => {
    setDisplayScale(transformRef.current.scale);
  }, []);

  const resetZoom = useCallback(() => {
    clearTimeout(wheelEndTimerRef.current);
    transformRef.current = { scale: 1, panX: 0, panY: 0 };
    applyTransform();
    setDisplayScale(1);
  }, [applyTransform]);

  useEffect(() => {
    resetZoom();
  }, [imagePath, resetZoom]);

  useEffect(() => {
    const handleResize = () => {
      resetZoom();
    };

    const unlisten = appWindow.onResized(handleResize);

    return () => {
      void unlisten.then(fn => fn()).catch(() => {});
    };
  }, [appWindow, resetZoom]);

  // Cleanup debounce timer on unmount
  useEffect(() => {
    return () => clearTimeout(wheelEndTimerRef.current);
  }, []);

  const handleWheel = useCallback((e: React.WheelEvent) => {
    e.preventDefault();

    const container = containerRef.current;
    const img = imgRef.current;
    if (!container || !img) return;

    const containerRect = container.getBoundingClientRect();
    const imgRect = img.getBoundingClientRect();

    const mouseX = e.clientX - containerRect.left;
    const mouseY = e.clientY - containerRect.top;

    const { scale, panX, panY } = transformRef.current;
    const zoomFactor = e.deltaY > 0 ? 0.9 : 1.1;
    const newScale = Math.max(1, Math.min(5, scale * zoomFactor));

    if (newScale === scale) {
      return;
    }

    const currentImgWidth = imgRect.width;
    const currentImgHeight = imgRect.height;

    const imgCenterX = imgRect.left + currentImgWidth / 2 - containerRect.left;
    const imgCenterY = imgRect.top + currentImgHeight / 2 - containerRect.top;

    const mouseOffsetX = mouseX - imgCenterX;
    const mouseOffsetY = mouseY - imgCenterY;

    const scaleRatio = newScale / scale;
    const newPanX = panX - mouseOffsetX * (scaleRatio - 1);
    const newPanY = panY - mouseOffsetY * (scaleRatio - 1);

    transformRef.current = {
      scale: newScale,
      panX: newScale > 1 ? newPanX : 0,
      panY: newScale > 1 ? newPanY : 0,
    };

    applyTransform();

    // Sync to React state only after zoom settles — avoids per-frame re-renders
    clearTimeout(wheelEndTimerRef.current);
    wheelEndTimerRef.current = setTimeout(syncDisplayScale, 150);
  }, [applyTransform, syncDisplayScale]);

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    if (transformRef.current.scale <= 1) {
      return;
    }

    isDraggingRef.current = true;
    setIsDragging(true);
    applyTransform();
    dragStartRef.current = {
      x: e.clientX - transformRef.current.panX,
      y: e.clientY - transformRef.current.panY,
    };
  }, [applyTransform]);

  const handleMouseMove = useCallback((e: React.MouseEvent) => {
    if (isDraggingRef.current && transformRef.current.scale > 1) {
      transformRef.current.panX = e.clientX - dragStartRef.current.x;
      transformRef.current.panY = e.clientY - dragStartRef.current.y;
      applyTransform();
    }
  }, [applyTransform]);

  const stopDragging = useCallback(() => {
    if (isDraggingRef.current) {
      isDraggingRef.current = false;
      setIsDragging(false);
      applyTransform();
    }
  }, [applyTransform]);

  return {
    scale: displayScale,
    isDragging,
    containerRef,
    imgRef,
    resetZoom,
    handleWheel,
    handleMouseDown,
    handleMouseMove,
    stopDragging,
  };
}
