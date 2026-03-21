/**
 * CameraFTP - A Cross-platform FTP companion for camera photo transfer
 * Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
 * SPDX-License-Identifier: AGPL-3.0-or-later
 */

import { useEffect, useState, useRef, useCallback, useMemo, memo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { convertFileSrc } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import type { ExifInfo } from '../types';
import type { ConfigChangedEvent } from '../types/events';
import { useConfigStore } from '../stores/configStore';
import { PREVIEW_NAVIGATE_EVENT } from '../hooks/preview-window-events';
import { usePreviewWindowLifecycle } from '../hooks/usePreviewWindowLifecycle';
import { usePreviewNavigation } from '../hooks/usePreviewNavigation';

export function PreviewWindow() {
  const state = usePreviewWindowLifecycle();

  // 实际预览窗口内容组件
  if (!state.isOpen) {
    return (
      <div className="w-full h-full flex items-center justify-center bg-black">
        <p className="text-gray-400">等待图片...</p>
      </div>
    );
  }

  return (
    <PreviewWindowContent
      imagePath={state.currentImage}
      autoBringToFront={state.autoBringToFront}
    />
  );
}

// 预览窗口内容组件
const PreviewWindowContent = memo(function PreviewWindowContent({
  imagePath,
  autoBringToFront,
}: {
  imagePath: string | null;
  autoBringToFront: boolean;
}) {
  const updatePreviewConfig = useConfigStore(state => state.updatePreviewConfig);
  const [showToolbar, setShowToolbar] = useState(true);
  const [isToolbarHovered, setIsToolbarHovered] = useState(false);
  const [imageError, setImageError] = useState(false);
  const [localAutoBringToFront, setLocalAutoBringToFront] = useState(autoBringToFront);
  const [isFullscreen, setIsFullscreen] = useState(false);
  const toolbarTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // EXIF 信息
  const [exifInfo, setExifInfo] = useState<ExifInfo | null>(null);

  // 加载 EXIF 信息
  const loadExifInfo = useCallback(async (path: string) => {
    try {
      const exif = await invoke<ExifInfo | null>('get_image_exif', { filePath: path });
      setExifInfo(exif);
    } catch {
      // Silently ignore - EXIF is optional metadata
      setExifInfo(null);
    }
  }, []);

  // 缩放和拖拽状态
  const [scale, setScale] = useState(1);
  const [panX, setPanX] = useState(0);
  const [panY, setPanY] = useState(0);
  const [isDragging, setIsDragging] = useState(false);
  const dragStartRef = useRef({ x: 0, y: 0 });
  const containerRef = useRef<HTMLDivElement>(null);
  const appWindow = getCurrentWindow();

  // 重置缩放
  const resetZoom = useCallback(() => {
    setScale(1);
    setPanX(0);
    setPanY(0);
  }, []);

  const {
    currentIndex,
    totalFiles,
    goToPrevious,
    goToNext,
    goToOldest,
    goToLatest,
  } = usePreviewNavigation({
    imagePath,
    onImagePathChange: (path) => {
      window.dispatchEvent(new CustomEvent(PREVIEW_NAVIGATE_EVENT, { detail: path }));
    },
    onBeforeNavigation: () => {
      setImageError(false);
    },
    onNavigationSettled: resetZoom,
  });

  // 同步外部状态
  useEffect(() => {
    setLocalAutoBringToFront(autoBringToFront);
  }, [autoBringToFront]);

  // 监听全局配置变化事件
  useEffect(() => {
    const setupListener = async () => {
      const unlisten = await listen<ConfigChangedEvent>('preview-config-changed', (event) => {
        setLocalAutoBringToFront(event.payload.config.autoBringToFront);
      });
      return unlisten;
    };

    const unlistenPromise = setupListener();
    return () => {
      void unlistenPromise.then(unlisten => unlisten()).catch(() => {});
    };
  }, []);

  // 自动隐藏工具栏（鼠标悬停在工具栏上时不隐藏）
  useEffect(() => {
    // 如果工具栏隐藏或鼠标悬停在工具栏上，不设置定时器
    if (!showToolbar || isToolbarHovered) {
      return;
    }

    if (toolbarTimeoutRef.current) {
      clearTimeout(toolbarTimeoutRef.current);
    }

    toolbarTimeoutRef.current = setTimeout(() => {
      setShowToolbar(false);
    }, 3000);

    return () => {
      if (toolbarTimeoutRef.current) {
        clearTimeout(toolbarTimeoutRef.current);
      }
    };
  }, [showToolbar, isToolbarHovered]);

  // 重置图片错误状态和缩放
  useEffect(() => {
    setImageError(false);
    resetZoom();
  }, [imagePath]);

  // 加载 EXIF 信息
  useEffect(() => {
    if (imagePath) {
      loadExifInfo(imagePath);
    }
  }, [imagePath]);

  // 监听窗口大小变化，重置缩放
  useEffect(() => {
    const handleResize = () => {
      resetZoom();
    };

    // 使用 Tauri 的窗口大小变化监听
    const unlisten = appWindow.onResized(handleResize);
    
    return () => {
      void unlisten.then(fn => fn()).catch(() => {});
    };
  }, [appWindow]);

  // 监听全屏状态变化
  useEffect(() => {
    // 初始检查
    void appWindow.isFullscreen().then(setIsFullscreen).catch(() => {});

    // 监听窗口大小变化来检测全屏状态变化
    const unlisten = appWindow.onResized(async () => {
      const fullscreen = await appWindow.isFullscreen();
      setIsFullscreen(fullscreen);
    });

    return () => {
      void unlisten.then(fn => fn()).catch(() => {});
    };
  }, [appWindow]);

  // 切换全屏
  const toggleFullscreen = useCallback(async () => {
    try {
      const newFullscreen = !isFullscreen;
      await appWindow.setFullscreen(newFullscreen);
      // 全屏时置顶，退出全屏时取消置顶
      await appWindow.setAlwaysOnTop(newFullscreen);
    } catch {
      // Silently ignore - fullscreen is a user preference
    }
  }, [isFullscreen, appWindow]);

  // 处理鼠标滚轮缩放
  const handleWheel = useCallback((e: React.WheelEvent) => {
    e.preventDefault();
    
    const container = containerRef.current;
    const img = container?.querySelector('img');
    if (!container || !img) return;

    const containerRect = container.getBoundingClientRect();
    const imgRect = img.getBoundingClientRect();
    
    // 鼠标相对于容器的位置
    const mouseX = e.clientX - containerRect.left;
    const mouseY = e.clientY - containerRect.top;
    
    // 计算缩放因子 - 最小为1（不裁切充满窗口）
    const zoomFactor = e.deltaY > 0 ? 0.9 : 1.1;
    const newScale = Math.max(1, Math.min(5, scale * zoomFactor));

    if (newScale !== scale) {
      // 计算图片当前实际显示尺寸（考虑object-contain）
      const currentImgWidth = imgRect.width;
      const currentImgHeight = imgRect.height;
      
      // 图片中心相对于容器中心的位置
      const imgCenterX = imgRect.left + currentImgWidth / 2 - containerRect.left;
      const imgCenterY = imgRect.top + currentImgHeight / 2 - containerRect.top;
      
      // 鼠标相对于图片中心的位置
      const mouseOffsetX = mouseX - imgCenterX;
      const mouseOffsetY = mouseY - imgCenterY;
      
      // 以鼠标位置为中心缩放的平移计算
      const scaleRatio = newScale / scale;
      const newPanX = panX - mouseOffsetX * (scaleRatio - 1);
      const newPanY = panY - mouseOffsetY * (scaleRatio - 1);

      setScale(newScale);
      // 只有在缩放大于1时才允许有平移
      if (newScale > 1) {
        setPanX(newPanX);
        setPanY(newPanY);
      } else {
        setPanX(0);
        setPanY(0);
      }
    }
  }, [scale, panX, panY]);

  // 处理鼠标按下（开始拖拽）
  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    if (scale > 1) {
      setIsDragging(true);
      dragStartRef.current = {
        x: e.clientX - panX,
        y: e.clientY - panY,
      };
    }
  }, [scale, panX, panY]);

  // 处理鼠标移动（拖拽中）
  const handleMouseMove = useCallback((e: React.MouseEvent) => {
    // 更新工具栏显示
    setShowToolbar(true);

    if (isDragging && scale > 1) {
      setPanX(e.clientX - dragStartRef.current.x);
      setPanY(e.clientY - dragStartRef.current.y);
    }
  }, [isDragging, scale]);

  // 处理鼠标释放（结束拖拽）
  const handleMouseUp = useCallback(() => {
    setIsDragging(false);
  }, []);

  // 处理双击重置
  const handleDoubleClick = useCallback(() => {
    resetZoom();
  }, [resetZoom]);

  // 全局键盘和鼠标释放监听
  useEffect(() => {
    const handleGlobalMouseUp = () => setIsDragging(false);

    const handleKeyDown = async (e: KeyboardEvent) => {
      switch (e.key) {
        case 'ArrowLeft':
        case 'ArrowUp':
          goToPrevious();
          break;
        case 'ArrowRight':
        case 'ArrowDown':
          goToNext();
          break;
        case 'Home':
          goToOldest();
          break;
        case 'End':
          goToLatest();
          break;
        case 'Escape':
          if (isFullscreen) {
            await appWindow.setFullscreen(false);
            await appWindow.setAlwaysOnTop(false);
          }
          break;
      }
    };

    window.addEventListener('mouseup', handleGlobalMouseUp);
    window.addEventListener('keydown', handleKeyDown);

    return () => {
      window.removeEventListener('mouseup', handleGlobalMouseUp);
      window.removeEventListener('keydown', handleKeyDown);
    };
  }, [isFullscreen, appWindow, goToPrevious, goToNext, goToLatest, goToOldest]);

  const handleOpenFolder = async () => {
    if (imagePath) {
      await invoke('open_folder_select_file', { filePath: imagePath });
    }
  };

  const handleToggleAutoFront = async () => {
    try {
      const newValue = !localAutoBringToFront;
      const savedConfig = await updatePreviewConfig({ autoBringToFront: newValue });
      if (savedConfig) {
        setLocalAutoBringToFront(savedConfig.autoBringToFront);
      }
    } catch {
      // Silently ignore - config change is not critical
    }
  };

  if (!imagePath) {
    return (
      <div className="w-full h-full flex items-center justify-center bg-black">
        <p className="text-gray-400">等待图片...</p>
      </div>
    );
  }

  // 使用 convertFileSrc 将文件路径转换为可用的 URL
  const imageSrc = convertFileSrc(imagePath);

  // 提取文件名（memoized）
  const fileName = useMemo(() => {
    return imagePath ? imagePath.split(/[/\\]/).pop() || '' : '';
  }, [imagePath]);

  return (
    <div
      className="w-full h-screen relative bg-black overflow-hidden"
    >
      {/* 图片区域 - 支持缩放和拖拽 */}
      <div 
        ref={containerRef}
        className={`absolute inset-0 flex items-center justify-center bg-black ${
          isDragging ? 'cursor-grabbing' : scale > 1 ? 'cursor-grab' : 'cursor-default'
        }`}
        onWheel={handleWheel}
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
        onDoubleClick={handleDoubleClick}
      >
        {imageError ? (
          <div className="text-gray-400 text-center">
            <svg className="w-16 h-16 mx-auto mb-4 text-gray-600" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M4 16l4.586-4.586a2 2 0 012.828 0L16 16m-2-2l1.586-1.586a2 2 0 012.828 0L20 14m-6-6h.01M6 20h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z" />
            </svg>
            <p>图片文件不存在或无法访问</p>
            <p className="text-xs mt-2 text-gray-500">{imagePath}</p>
            <p className="text-sm mt-3 text-gray-500">
              文件可能已被删除，请使用导航按钮切换到其他图片
            </p>
          </div>
        ) : (
          <img
            src={imageSrc}
            alt="Preview"
            className="max-w-full max-h-full object-contain select-none"
            style={{
              transform: `translate(${panX}px, ${panY}px) scale(${scale})`,
              transformOrigin: 'center center',
            }}
            draggable={false}
            onError={() => setImageError(true)}
          />
        )}
      </div>

      {/* 底部工具栏 - 浮动覆盖在图片上，半透明磨砂效果 */}
      <div
        className={`
          absolute bottom-4 left-4 right-4
          bg-gray-900/80 backdrop-blur-md
          border border-gray-700/50
          rounded-xl
          px-4 py-3 flex items-center
          shadow-lg
          transition-all duration-300
          ${showToolbar ? 'opacity-100 translate-y-0' : 'opacity-0 translate-y-4 pointer-events-none'}
        `}
        onMouseEnter={() => setIsToolbarHovered(true)}
        onMouseLeave={() => setIsToolbarHovered(false)}
      >
        {/* 左侧：文件名和拍摄信息 */}
        <div className="flex items-center gap-3 min-w-0">
          {/* 文件名 - 跨两行 */}
          <div className="flex flex-col justify-center min-w-0">
            <span className="text-sm text-gray-200 truncate">
              {fileName}
            </span>
          </div>
          {/* 竖线分隔符 - 跨两行高度 */}
          {exifInfo && (
            <div className="w-px h-8 bg-gray-600 mx-1"></div>
          )}
          {/* 拍摄信息 - 双行布局 */}
          {exifInfo && (
            <div className="flex flex-col text-xs text-gray-400 gap-0.5">
              {/* 第一行：ISO | 光圈 | 快门速度 | 焦距 */}
              <div className="flex items-center gap-2">
                {exifInfo.iso !== undefined && (
                  <span className="flex items-center gap-2">
                    ISO {exifInfo.iso}
                    <svg className="w-1 h-1 text-gray-600" fill="currentColor" viewBox="0 0 4 4"><circle cx="2" cy="2" r="2"/></svg>
                  </span>
                )}
                {exifInfo.aperture && (
                  <span className="flex items-center gap-2">
                    {exifInfo.aperture}
                    <svg className="w-1 h-1 text-gray-600" fill="currentColor" viewBox="0 0 4 4"><circle cx="2" cy="2" r="2"/></svg>
                  </span>
                )}
                {exifInfo.shutterSpeed && (
                  <span className="flex items-center gap-2">
                    {exifInfo.shutterSpeed}
                    <svg className="w-1 h-1 text-gray-600" fill="currentColor" viewBox="0 0 4 4"><circle cx="2" cy="2" r="2"/></svg>
                  </span>
                )}
                {exifInfo.focalLength && (
                  <span>{exifInfo.focalLength}</span>
                )}
              </div>
              {/* 第二行：拍摄时间 */}
              {exifInfo.datetime && (
                <span className="text-gray-500">{exifInfo.datetime}</span>
              )}
            </div>
          )}
        </div>

        {/* 中间：导航按钮 - 绝对居中 */}
        <div className="absolute left-1/2 -translate-x-1/2 flex items-center">
          <div className="flex items-center gap-1">
            {/* 最旧 */}
            <button
              onClick={goToOldest}
              disabled={totalFiles <= 1 || currentIndex >= totalFiles - 1}
              className="p-2 text-gray-300 hover:text-white hover:bg-white/10 rounded-lg transition-colors disabled:opacity-30 disabled:cursor-not-allowed"
              title="最旧 (Home)"
            >
              <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M11 19l-7-7 7-7m8 14l-7-7 7-7" />
              </svg>
            </button>

            {/* 上一张 */}
            <button
              onClick={goToPrevious}
              disabled={totalFiles <= 1 || currentIndex >= totalFiles - 1}
              className="p-2 text-gray-300 hover:text-white hover:bg-white/10 rounded-lg transition-colors disabled:opacity-30 disabled:cursor-not-allowed"
              title="上一张 (← ↑)"
            >
              <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 19l-7-7 7-7" />
              </svg>
            </button>

            {/* 下一张 */}
            <button
              onClick={goToNext}
              disabled={totalFiles <= 1 || currentIndex <= 0}
              className="p-2 text-gray-300 hover:text-white hover:bg-white/10 rounded-lg transition-colors disabled:opacity-30 disabled:cursor-not-allowed"
              title="下一张 (→ ↓)"
            >
              <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 5l7 7-7 7" />
              </svg>
            </button>

            {/* 最新 */}
            <button
              onClick={goToLatest}
              disabled={totalFiles <= 1 || currentIndex <= 0}
              className="p-2 text-gray-300 hover:text-white hover:bg-white/10 rounded-lg transition-colors disabled:opacity-30 disabled:cursor-not-allowed"
              title="最新 (End)"
            >
              <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M13 5l7 7-7 7M5 5l7 7-7 7" />
              </svg>
            </button>
          </div>
        </div>

        {/* 右侧：操作按钮 */}
        <div className="flex items-center gap-2 ml-auto">
          {/* 缩放比例 - 在放大镜图标左侧 */}
          {scale !== 1 && (
            <span className="text-xs text-blue-300 bg-blue-500/20 px-2 py-0.5 rounded">
              {Math.round(scale * 100)}%
            </span>
          )}
          {/* 重置缩放按钮 - 放大镜图标（缩放状态下高亮） */}
          {scale !== 1 && (
            <button
              onClick={resetZoom}
              className="p-2 text-blue-300 bg-blue-500/20 hover:bg-blue-500/30 rounded-lg transition-colors"
              title="重置缩放 (双击图片也可重置)"
            >
              <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <circle cx="11" cy="11" r="7" strokeWidth="2" />
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M20 20l-4.35-4.35" />
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M11 8v6M8 11h6" />
              </svg>
            </button>
          )}

          {/* 全屏按钮 */}
          <button
            onClick={toggleFullscreen}
            className="p-2 text-gray-300 hover:text-white hover:bg-white/10 rounded-lg transition-colors"
            title={isFullscreen ? '退出全屏' : '全屏显示'}
          >
            {isFullscreen ? (
              <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M9 9V4.5M9 9H4.5M9 9L3.75 3.75M15 9h4.5M15 9V4.5M15 9l5.25-5.25M9 15v4.5M9 15H4.5M9 15l-5.25 5.25M15 15h4.5M15 15v4.5m0-4.5l5.25 5.25" />
              </svg>
            ) : (
              <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3.75 3.75v4.5m0-4.5h4.5m-4.5 0L9 9M3.75 20.25v-4.5m0 4.5h4.5m-4.5 0L9 15M20.25 3.75h-4.5m4.5 0v4.5m0-4.5L15 9m5.25 11.25h-4.5m4.5 0v-4.5m0 4.5L15 15" />
              </svg>
            )}
          </button>

          {/* 自动前台按钮 - 使用置顶图标（向上箭头指向横线） */}
          <button
            onClick={handleToggleAutoFront}
            className={`
              p-2 rounded-lg transition-colors
              ${localAutoBringToFront
                ? 'text-blue-300 bg-blue-500/20 hover:bg-blue-500/30'
                : 'text-gray-300 hover:text-white hover:bg-white/10'
              }
            `}
            title={localAutoBringToFront ? '新图片时自动前台显示 (已开启)' : '新图片时自动前台显示 (已关闭)'}
          >
            <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24" strokeWidth="2">
              <path strokeLinecap="round" strokeLinejoin="round" d="M12 20V10M12 10l-5 5M12 10l5 5" />
              <path strokeLinecap="round" d="M5 6h14" />
            </svg>
          </button>

          {/* 打开文件夹 */}
          <button
            onClick={handleOpenFolder}
            className="p-2 text-gray-300 hover:text-white hover:bg-white/10 rounded-lg transition-colors"
            title="打开文件夹"
          >
            <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z" />
            </svg>
          </button>
        </div>
      </div>
    </div>
  );
});
