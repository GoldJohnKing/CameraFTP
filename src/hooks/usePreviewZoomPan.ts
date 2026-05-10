/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';

export function usePreviewZoomPan(imagePath: string | null) {
  // Continuous interaction state in refs — bypasses React rendering pipeline
  const transformRef = useRef({ scale: 1, panX: 0, panY: 0 });
  const imgRef = useRef<HTMLImageElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const isDraggingRef = useRef(false);
  const dragStartRef = useRef({ x: 0, y: 0 });
  const appWindow = useMemo(() => getCurrentWindow(), []);

  // React state only for UI display (toolbar, cursor class)
  const [displayScale, setDisplayScale] = useState(1);
  const [isDragging, setIsDragging] = useState(false);

  // Apply transform directly to <img> DOM element, no React re-render
  const applyTransform = useCallback(() => {
    const img = imgRef.current;
    if (img) {
      const { scale, panX, panY } = transformRef.current;
      img.style.transform = `translate(${panX}px, ${panY}px) scale(${scale})`;
    }
  }, []);

  const resetZoom = useCallback(() => {
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
    setDisplayScale(newScale);
  }, [applyTransform]);

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    if (transformRef.current.scale <= 1) {
      return;
    }

    isDraggingRef.current = true;
    setIsDragging(true);
    dragStartRef.current = {
      x: e.clientX - transformRef.current.panX,
      y: e.clientY - transformRef.current.panY,
    };
  }, []);

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
    }
  }, []);

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
