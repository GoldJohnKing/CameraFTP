/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';

export function usePreviewZoomPan(imagePath: string | null) {
  const [scale, setScale] = useState(1);
  const [panX, setPanX] = useState(0);
  const [panY, setPanY] = useState(0);
  const [isDragging, setIsDragging] = useState(false);
  const dragStartRef = useRef({ x: 0, y: 0 });
  const containerRef = useRef<HTMLDivElement>(null);
  const isDraggingRef = useRef(false);
  const scaleRef = useRef(1);
  const appWindow = useMemo(() => getCurrentWindow(), []);

  // Keep refs in sync with state for stable callbacks
  isDraggingRef.current = isDragging;
  scaleRef.current = scale;

  const resetZoom = useCallback(() => {
    setScale(1);
    setPanX(0);
    setPanY(0);
  }, []);

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
    const img = container?.querySelector('img');
    if (!container || !img) return;

    const containerRect = container.getBoundingClientRect();
    const imgRect = img.getBoundingClientRect();

    const mouseX = e.clientX - containerRect.left;
    const mouseY = e.clientY - containerRect.top;

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

    setScale(newScale);
    if (newScale > 1) {
      setPanX(newPanX);
      setPanY(newPanY);
    } else {
      setPanX(0);
      setPanY(0);
    }
  }, [scale, panX, panY]);

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    if (scale <= 1) {
      return;
    }

    setIsDragging(true);
    dragStartRef.current = {
      x: e.clientX - panX,
      y: e.clientY - panY,
    };
  }, [scale, panX, panY]);

  const handleMouseMove = useCallback((e: React.MouseEvent) => {
    if (isDraggingRef.current && scaleRef.current > 1) {
      setPanX(e.clientX - dragStartRef.current.x);
      setPanY(e.clientY - dragStartRef.current.y);
    }
  }, []);

  const stopDragging = useCallback(() => {
    setIsDragging(false);
  }, []);

  return {
    scale,
    panX,
    panY,
    isDragging,
    containerRef,
    resetZoom,
    handleWheel,
    handleMouseDown,
    handleMouseMove,
    stopDragging,
  };
}
